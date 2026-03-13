use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use anyhow::{Result, anyhow};
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
};
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use tokio::{
    net::TcpListener,
    sync::{Mutex, oneshot},
    task::JoinHandle,
};

use crate::{
    debug::debug_log,
    types::{AuthWindowSession, LoginInput},
};

#[derive(Debug, Clone, Default)]
pub struct LocalAuthDefaults {
    pub base_url: Option<String>,
    pub email: Option<String>,
    pub base_url_readonly: bool,
}

type SubmitHandler = Arc<dyn Fn(LoginInput) -> BoxFuture<'static, Result<()>> + Send + Sync>;

#[derive(Clone)]
struct AppState {
    defaults: LocalAuthDefaults,
    on_submit: SubmitHandler,
    shared: Arc<SharedState>,
}

struct SharedState {
    settled: AtomicBool,
    completion_tx: Mutex<Option<oneshot::Sender<Result<(), String>>>>,
    shutdown_tx: Mutex<Option<oneshot::Sender<()>>>,
}

pub struct LocalAuthServer {
    defaults: LocalAuthDefaults,
    on_submit: SubmitHandler,
    timeout_ms: u64,
    shared: Arc<SharedState>,
    completion_rx: Option<oneshot::Receiver<Result<(), String>>>,
    server_task: Option<JoinHandle<Result<()>>>,
    timeout_task: Option<JoinHandle<()>>,
}

impl LocalAuthServer {
    pub fn new<F, Fut>(defaults: LocalAuthDefaults, on_submit: F, timeout_ms: Option<u64>) -> Self
    where
        F: Fn(LoginInput) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<()>> + Send + 'static,
    {
        let (completion_tx, completion_rx) = oneshot::channel();

        Self {
            defaults,
            on_submit: Arc::new(move |input| Box::pin(on_submit(input))),
            timeout_ms: timeout_ms.unwrap_or(5 * 60 * 1000),
            shared: Arc::new(SharedState {
                settled: AtomicBool::new(false),
                completion_tx: Mutex::new(Some(completion_tx)),
                shutdown_tx: Mutex::new(None),
            }),
            completion_rx: Some(completion_rx),
            server_task: None,
            timeout_task: None,
        }
    }

    pub async fn start(&mut self) -> Result<AuthWindowSession> {
        if self.server_task.is_some() {
            return Err(anyhow!("Local auth server is already running."));
        }

        let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        let address = listener.local_addr()?;
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        *self.shared.shutdown_tx.lock().await = Some(shutdown_tx);

        let state = AppState {
            defaults: self.defaults.clone(),
            on_submit: self.on_submit.clone(),
            shared: self.shared.clone(),
        };

        let router = Router::new()
            .route("/", get(root))
            .route("/login", get(login))
            .route("/auth", post(auth))
            .route("/success", get(success))
            .with_state(state);

        self.server_task = Some(tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .map_err(Into::into)
        }));

        let timeout_shared = self.shared.clone();
        let timeout_ms = self.timeout_ms;
        self.timeout_task = Some(tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(timeout_ms)).await;
            let _ = finish(
                timeout_shared,
                Err("Timed out waiting for Docmost sign-in to complete.".to_string()),
            )
            .await;
        }));

        let url = format!("http://127.0.0.1:{}", address.port());
        debug_log(
            "local-auth",
            "Local auth page ready",
            Some(&serde_json::json!({ "url": url, "defaults": {
                "baseUrl": self.defaults.base_url,
                "email": self.defaults.email,
                "baseUrlReadonly": self.defaults.base_url_readonly
            }})),
        );

        Ok(AuthWindowSession {
            login_url: format!("{url}/login"),
            success_url: format!("{url}/success"),
            fallback_url: format!("{url}/login"),
            window_title: "Docmost Sign In".to_string(),
            window_width: 500,
            window_height: 680,
        })
    }

    pub async fn wait_for_completion(&mut self) -> Result<()> {
        let Some(receiver) = self.completion_rx.take() else {
            return Err(anyhow!("Local auth server has not been started."));
        };

        match receiver.await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(message)) => Err(anyhow!(message)),
            Err(_) => Err(anyhow!(
                "Local auth server completion channel was closed unexpectedly."
            )),
        }
    }

    pub async fn stop(&mut self) -> Result<()> {
        if let Some(timeout_task) = self.timeout_task.take() {
            timeout_task.abort();
        }

        if let Some(shutdown_tx) = self.shared.shutdown_tx.lock().await.take() {
            let _ = shutdown_tx.send(());
        }

        if let Some(server_task) = self.server_task.take() {
            server_task.await??;
        }

        Ok(())
    }
}

async fn root() -> impl IntoResponse {
    Redirect::temporary("/login")
}

async fn login(State(state): State<AppState>) -> impl IntoResponse {
    Html(render_login_html(&state.defaults))
}

async fn success(State(state): State<AppState>) -> impl IntoResponse {
    let shared = state.shared.clone();
    tokio::spawn(async move {
        let _ = finish(shared, Ok(())).await;
    });
    Html(render_success_html())
}

async fn auth(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<PartialLoginInput>,
) -> impl IntoResponse {
    debug_log(
        "local-auth",
        "Received auth form submission",
        Some(&serde_json::json!({
            "contentType": headers.get("content-type").and_then(|v| v.to_str().ok()).unwrap_or(""),
        })),
    );

    let parsed = match parse_login_input(payload, &state.defaults) {
        Ok(parsed) => parsed,
        Err(error) => {
            debug_log(
                "local-auth",
                "Auth submission failed",
                Some(&serde_json::json!({ "error": error.to_string() })),
            );
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthResponse::error(error.to_string())),
            );
        }
    };

    match (state.on_submit)(parsed.clone()).await {
        Ok(()) => {
            debug_log(
                "local-auth",
                "Auth submission succeeded",
                Some(&serde_json::json!({ "baseUrl": parsed.base_url, "email": parsed.email })),
            );
            let shared = state.shared.clone();
            tokio::spawn(async move {
                let _ = finish(shared, Ok(())).await;
            });
            (
                StatusCode::OK,
                Json(AuthResponse::success("/success".to_string())),
            )
        }
        Err(error) => {
            debug_log(
                "local-auth",
                "Auth submission failed",
                Some(&serde_json::json!({ "error": error.to_string() })),
            );
            (
                StatusCode::BAD_REQUEST,
                Json(AuthResponse::error(error.to_string())),
            )
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PartialLoginInput {
    base_url: Option<String>,
    email: Option<String>,
    password: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthResponse {
    ok: bool,
    redirect_url: Option<String>,
    error: Option<String>,
}

impl AuthResponse {
    fn success(redirect_url: String) -> Self {
        Self {
            ok: true,
            redirect_url: Some(redirect_url),
            error: None,
        }
    }

    fn error(error: String) -> Self {
        Self {
            ok: false,
            redirect_url: None,
            error: Some(error),
        }
    }
}

fn parse_login_input(raw: PartialLoginInput, defaults: &LocalAuthDefaults) -> Result<LoginInput> {
    let base_url = raw
        .base_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| defaults.base_url.clone());
    let email = raw
        .email
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let password = raw.password.filter(|value| !value.is_empty());

    match (base_url, email, password) {
        (Some(base_url), Some(email), Some(password)) => Ok(LoginInput {
            base_url,
            email,
            password,
        }),
        _ => Err(anyhow!("Base URL, email, and password are required.")),
    }
}

async fn finish(shared: Arc<SharedState>, outcome: Result<(), String>) -> Result<()> {
    if shared.settled.swap(true, Ordering::SeqCst) {
        return Ok(());
    }

    if let Some(sender) = shared.completion_tx.lock().await.take() {
        let _ = sender.send(outcome);
    }

    if let Some(shutdown_tx) = shared.shutdown_tx.lock().await.take() {
        let _ = shutdown_tx.send(());
    }

    Ok(())
}

fn render_success_html() -> String {
    r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Docmost MCP</title>
    <style>
      :root { color-scheme: dark; }
      body {
        margin: 0; min-height: 100vh; display: grid; place-items: center;
        background: linear-gradient(180deg, #0b1020 0%, #080c18 100%);
        font-family: ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
        color: #aab4d6;
      }
      .msg { text-align: center; max-width: 460px; line-height: 1.6; }
      h2 { color: #f4f7ff; margin-bottom: 8px; }
      a { color: #7dd3fc; }
    </style>
  </head>
  <body>
    <div class="msg">
      <h2>Authentication Succeeded</h2>
      <p>This window can close now.</p>
    </div>
    <script>
      setTimeout(() => {
        try { window.close(); } catch {}
      }, 400);
    </script>
  </body>
</html>"#
        .to_string()
}

fn render_login_html(defaults: &LocalAuthDefaults) -> String {
    let base_url = escape_html(defaults.base_url.as_deref().unwrap_or_default());
    let email = escape_html(defaults.email.as_deref().unwrap_or_default());
    let readonly_base_url = if defaults.base_url_readonly {
        "readonly"
    } else {
        ""
    };
    let base_url_hint = if defaults.base_url_readonly {
        "Configured by the MCP server startup options."
    } else {
        "Use the full Docmost URL, for example https://docs.example.com."
    };

    format!(
        r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Docmost MCP Sign In</title>
    <style>
      :root {{
        color-scheme: dark;
        --bg: #0b1020;
        --panel: #141b34;
        --panel-border: #2d3763;
        --text: #f4f7ff;
        --muted: #aab4d6;
        --accent: #7dd3fc;
        --accent-strong: #38bdf8;
        --danger: #fca5a5;
      }}
      * {{ box-sizing: border-box; }}
      body {{
        margin: 0;
        min-height: 100vh;
        display: grid;
        place-items: center;
        background:
          radial-gradient(circle at top, rgba(56, 189, 248, 0.18), transparent 35%),
          linear-gradient(180deg, #0b1020 0%, #080c18 100%);
        font-family: ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
        color: var(--text);
      }}
      .card {{
        width: min(92vw, 480px);
        padding: 28px;
        border-radius: 20px;
        border: 1px solid var(--panel-border);
        background: rgba(20, 27, 52, 0.92);
        box-shadow: 0 18px 45px rgba(0, 0, 0, 0.32);
      }}
      h1 {{ margin: 0 0 12px; font-size: 1.6rem; }}
      p {{ margin: 0 0 16px; color: var(--muted); line-height: 1.5; font-size: 0.9rem; }}
      label {{ display: block; margin-bottom: 12px; font-size: 0.92rem; }}
      input {{
        width: 100%; margin-top: 5px; padding: 10px 12px; border-radius: 10px;
        border: 1px solid #3b4a85; background: #0b1227; color: var(--text); font-size: 0.92rem;
      }}
      button {{
        width: 100%; margin-top: 8px; padding: 12px 14px; border: 0; border-radius: 10px;
        background: linear-gradient(135deg, var(--accent), var(--accent-strong));
        color: #04101a; font-weight: 700; cursor: pointer;
      }}
      button:disabled {{ opacity: 0.7; cursor: progress; }}
      .status {{ min-height: 22px; margin-top: 14px; color: var(--muted); }}
      .status.error {{ color: var(--danger); }}
      .status.success {{ color: #86efac; }}
    </style>
  </head>
  <body>
    <main class="card">
      <h1>Sign in to Docmost</h1>
      <p>
        Credentials are sent only to the local MCP process, which then signs in to your Docmost instance.
      </p>
      <form id="login-form">
        <label>
          Docmost Base URL
          <input id="baseUrl" name="baseUrl" value="{base_url}" placeholder="https://docs.example.com" {readonly_base_url} required />
        </label>
        <p>{base_url_hint}</p>
        <label>
          Email
          <input id="email" name="email" type="email" value="{email}" placeholder="you@example.com" required />
        </label>
        <label>
          Password
          <input id="password" name="password" type="password" placeholder="Your Docmost password" required />
        </label>
        <button id="submit-button" type="submit">Authenticate</button>
        <div id="status" class="status" role="status"></div>
      </form>
    </main>
    <script>
      const form = document.getElementById("login-form");
      const status = document.getElementById("status");
      const submitButton = document.getElementById("submit-button");

      form.addEventListener("submit", async (event) => {{
        event.preventDefault();
        submitButton.disabled = true;
        status.className = "status";
        status.textContent = "Signing in...";

        const payload = {{
          baseUrl: document.getElementById("baseUrl").value,
          email: document.getElementById("email").value,
          password: document.getElementById("password").value
        }};

        try {{
          const response = await fetch("/auth", {{
            method: "POST",
            headers: {{ "content-type": "application/json" }},
            body: JSON.stringify(payload)
          }});
          const result = await response.json();

          if (!response.ok || !result.ok) {{
            throw new Error(result.error || "Authentication failed");
          }}

          status.className = "status success";
          status.textContent = "Authenticated. Finishing sign-in...";
          window.location.assign(result.redirectUrl || "/success");
        }} catch (error) {{
          status.className = "status error";
          status.textContent = error instanceof Error ? error.message : String(error);
          submitButton.disabled = false;
        }}
      }});
    </script>
  </body>
</html>"#
    )
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
