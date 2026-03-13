use std::{path::PathBuf, sync::Arc};

use anyhow::{Context, Result, anyhow};
use base64::{
    Engine as _,
    engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD},
};
use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::{Client, Response, header::SET_COOKIE};

use crate::{
    auth::{
        local_server::{LocalAuthDefaults, LocalAuthServer},
        webview::{
            AuthWindowHandle, helper_exit_cancelled, helper_exit_error_message,
            helper_exit_success, launch_auth_window,
        },
    },
    debug::debug_log,
    startup_config::normalize_base_url,
    storage::state_store::StateStore,
    types::{
        AuthenticatedSession, LoginInput, StartupConfig, StoredConfig, StoredCredentials,
        StoredSession,
    },
};

const REFRESH_WINDOW_MS: i64 = 2 * 60 * 1000;
static AUTH_TOKEN_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?:^|,\s*)authToken=([^;]+)").expect("valid auth token regex"));

#[derive(Debug, Clone)]
pub struct AuthManager {
    store: Arc<StateStore>,
    configured_base_url: Option<String>,
    http: Client,
}

impl AuthManager {
    pub fn new(options: StartupConfig, base_dir: Option<PathBuf>) -> Result<Self> {
        Ok(Self {
            configured_base_url: options.base_url.as_deref().map(normalize_base_url),
            store: Arc::new(StateStore::new(base_dir)?),
            http: Client::builder().build()?,
        })
    }

    pub async fn get_authenticated_session(&self) -> Result<AuthenticatedSession> {
        let config = self.store.read_config().await?;
        let session = self.store.read_session().await?;
        let preferred_base_url = self
            .get_preferred_base_url(config.as_ref())
            .map(ToOwned::to_owned);
        let has_config = config.is_some();
        let has_session = session.is_some();

        if let (Some(config), Some(session)) = (config.as_ref(), session.as_ref()) {
            if preferred_base_url.as_deref() == Some(config.base_url.as_str())
                && !is_session_expiring(session)
            {
                debug_log(
                    "auth",
                    "Using saved session",
                    Some(&serde_json::json!({
                        "baseUrl": config.base_url,
                        "email": config.email,
                        "expiresAt": session.expires_at
                    })),
                );
                return Ok(to_authenticated_session(config.clone(), session.clone()));
            }
        }

        debug_log(
            "auth",
            "Saved session missing or expiring; reauthenticating",
            Some(&serde_json::json!({
                "hasConfig": has_config,
                "hasSession": has_session,
            })),
        );
        self.reauthenticate().await
    }

    pub async fn reauthenticate(&self) -> Result<AuthenticatedSession> {
        let config = self.store.read_config().await?;
        let credentials = self.store.read_credentials().await?;
        let preferred_base_url = self
            .get_preferred_base_url(config.as_ref())
            .map(ToOwned::to_owned);
        let has_config = config.is_some();
        let has_credentials = credentials.is_some();

        if let (Some(base_url), Some(credentials)) =
            (preferred_base_url.as_deref(), credentials.clone())
        {
            debug_log(
                "auth",
                "Reauthenticating with saved credentials",
                Some(&serde_json::json!({
                    "baseUrl": base_url,
                    "email": credentials.email
                })),
            );
            return self
                .login(LoginInput {
                    base_url: base_url.to_string(),
                    email: credentials.email,
                    password: credentials.password,
                })
                .await;
        }

        debug_log(
            "auth",
            "No reusable credentials available; starting interactive authentication",
            Some(&serde_json::json!({
                "hasConfig": has_config,
                "hasCredentials": has_credentials,
                "configuredBaseUrl": self.configured_base_url
            })),
        );
        self.prompt_for_login(config.as_ref()).await
    }

    pub async fn login(&self, input: LoginInput) -> Result<AuthenticatedSession> {
        let base_url = normalize_base_url(&input.base_url);
        debug_log(
            "auth",
            "Starting Docmost login",
            Some(&serde_json::json!({ "baseUrl": base_url, "email": input.email })),
        );

        let response = self
            .http
            .post(format!("{base_url}/api/auth/login"))
            .json(&serde_json::json!({
                "email": input.email,
                "password": input.password
            }))
            .send()
            .await
            .context("Failed to call the Docmost login endpoint")?;

        debug_log(
            "auth",
            "Docmost login response received",
            Some(&serde_json::json!({
                "status": response.status().as_u16(),
                "ok": response.status().is_success()
            })),
        );

        if !response.status().is_success() {
            let status = response.status();
            let details = safe_read_response_text(response).await;
            return Err(anyhow!(
                format!("Docmost login failed ({}). {}", status, details)
                    .trim()
                    .to_string()
            ));
        }

        let token = read_auth_token_from_headers(response.headers()).ok_or_else(|| {
            anyhow!("Docmost login succeeded but no authToken cookie was returned.")
        })?;
        let expires_at = get_jwt_expiry_iso(&token);
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

        self.store
            .write_config(&StoredConfig {
                base_url: base_url.clone(),
                email: input.email.clone(),
                last_authenticated_at: now.clone(),
            })
            .await?;
        self.store
            .write_session(&StoredSession {
                token: token.clone(),
                expires_at: expires_at.clone(),
                saved_at: now.clone(),
            })
            .await?;
        self.store
            .write_credentials(&StoredCredentials {
                email: input.email.clone(),
                password: input.password,
            })
            .await?;

        Ok(AuthenticatedSession {
            base_url,
            email: input.email,
            token,
            expires_at,
        })
    }

    fn get_preferred_base_url<'a>(&'a self, config: Option<&'a StoredConfig>) -> Option<&'a str> {
        self.configured_base_url
            .as_deref()
            .or_else(|| config.map(|config| config.base_url.as_str()))
    }

    async fn prompt_for_login(
        &self,
        config: Option<&StoredConfig>,
    ) -> Result<AuthenticatedSession> {
        let preferred_base_url = self.get_preferred_base_url(config);
        let defaults = LocalAuthDefaults {
            base_url: preferred_base_url.map(ToOwned::to_owned),
            email: config.map(|config| config.email.clone()),
            base_url_readonly: self.configured_base_url.is_some(),
        };

        let auth_manager = self.clone();
        let mut auth_server = LocalAuthServer::new(
            defaults,
            move |input| {
                let auth_manager = auth_manager.clone();
                async move {
                    auth_manager.login(input).await?;
                    Ok(())
                }
            },
            None,
        );

        let auth_session = auth_server.start().await?;
        let mut auth_window = launch_auth_window(&auth_session).await?;

        debug_log(
            "auth",
            "Waiting for interactive authentication",
            Some(&serde_json::json!({
                "mode": format!("{:?}", auth_window.mode),
                "loginUrl": auth_session.login_url
            })),
        );

        let completion =
            wait_for_authentication_completion(&mut auth_server, &mut auth_window).await;

        let result =
            async {
                completion?;
                let refreshed_config =
                    self.store.read_config().await?.ok_or_else(|| {
                        anyhow!("Authentication completed, but no config was saved.")
                    })?;
                let refreshed_session = self.store.read_session().await?.ok_or_else(|| {
                    anyhow!("Authentication completed, but no session was saved.")
                })?;
                Ok(to_authenticated_session(
                    refreshed_config,
                    refreshed_session,
                ))
            }
            .await;

        auth_window.close().await?;
        auth_server.stop().await?;

        result
    }
}

fn to_authenticated_session(config: StoredConfig, session: StoredSession) -> AuthenticatedSession {
    AuthenticatedSession {
        base_url: config.base_url,
        email: config.email,
        token: session.token,
        expires_at: session.expires_at,
    }
}

fn is_session_expiring(session: &StoredSession) -> bool {
    let Some(expires_at) = &session.expires_at else {
        return false;
    };

    let Ok(expires_at) = DateTime::parse_from_rfc3339(expires_at) else {
        return false;
    };

    expires_at.timestamp_millis() - Utc::now().timestamp_millis() <= REFRESH_WINDOW_MS
}

pub fn read_auth_token_from_headers(headers: &reqwest::header::HeaderMap) -> Option<String> {
    for header in headers.get_all(SET_COOKIE) {
        let Ok(cookie) = header.to_str() else {
            continue;
        };

        if let Some(captures) = AUTH_TOKEN_RE.captures(cookie) {
            if let Some(token) = captures.get(1) {
                let decoded = urlencoding::decode(token.as_str()).ok()?;
                return Some(decoded.into_owned());
            }
        }
    }

    None
}

pub fn get_jwt_expiry_iso(token: &str) -> Option<String> {
    let payload_part = token.split('.').nth(1)?;
    let decoded = URL_SAFE_NO_PAD
        .decode(payload_part)
        .or_else(|_| URL_SAFE.decode(payload_part))
        .ok()?;
    let payload = serde_json::from_slice::<serde_json::Value>(&decoded).ok()?;
    let exp = payload.get("exp")?.as_i64()?;
    DateTime::<Utc>::from_timestamp(exp, 0)
        .map(|value| value.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
}

pub async fn safe_read_response_text(response: Response) -> String {
    match response.text().await {
        Ok(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                String::new()
            } else {
                format!("Response: {trimmed}")
            }
        }
        Err(_) => String::new(),
    }
}

async fn wait_for_authentication_completion(
    auth_server: &mut LocalAuthServer,
    auth_window: &mut AuthWindowHandle,
) -> Result<()> {
    if auth_window.mode == crate::auth::webview::AuthWindowMode::Browser {
        return auth_server.wait_for_completion().await;
    }

    let completion = auth_server.wait_for_completion();
    tokio::pin!(completion);

    tokio::select! {
        result = &mut completion => result,
        exit = auth_window.wait_for_exit() => {
            let code = exit?;
            if helper_exit_success(code) {
                completion.await
            } else if helper_exit_cancelled(code) {
                Err(anyhow!("The Docmost sign-in window was closed before authentication completed."))
            } else {
                Err(anyhow!(helper_exit_error_message(code)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use tokio::{
        process::Command,
        time::{Duration, sleep, timeout},
    };

    use super::wait_for_authentication_completion;
    use crate::auth::{
        local_server::{LocalAuthDefaults, LocalAuthServer},
        webview::{AuthWindowHandle, AuthWindowMode},
    };

    #[tokio::test]
    async fn native_success_exit_still_waits_for_auth_completion() -> Result<()> {
        let mut auth_server = LocalAuthServer::new(
            LocalAuthDefaults::default(),
            |_input| async move { Ok(()) },
            Some(5_000),
        );
        let auth_session = auth_server.start().await?;

        let auth_url = auth_session.login_url.replacen("/login", "/auth", 1);
        tokio::spawn(async move {
            sleep(Duration::from_millis(50)).await;
            let _ = reqwest::Client::new()
                .post(&auth_url)
                .json(&serde_json::json!({
                    "baseUrl": "https://docs.example.com",
                    "email": "jane@example.com",
                    "password": "super-secret"
                }))
                .send()
                .await;
        });

        let child = Command::new(std::env::current_exe()?)
            .arg("--help")
            .spawn()?;
        let mut auth_window = AuthWindowHandle::test_handle(AuthWindowMode::Native, child);

        let result = timeout(
            Duration::from_secs(2),
            wait_for_authentication_completion(&mut auth_server, &mut auth_window),
        )
        .await?;

        auth_window.close().await?;
        auth_server.stop().await?;

        result
    }
}
