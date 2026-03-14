use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use axum::{
    Json, Router,
    extract::{Request, State},
    http::{HeaderValue, StatusCode, header::SET_COOKIE},
    response::IntoResponse,
    routing::post,
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use docmost_local_mcp::{
    auth::manager::{AuthManager, get_jwt_expiry_iso, read_auth_token_from_headers},
    docmost_client::DocmostClient,
    startup_config::normalize_base_url,
    storage::state_store::StateStore,
    types::{LoginInput, StartupConfig, StoredConfig, StoredCredentials, StoredSession},
};
use tempfile::TempDir;
use tokio::net::TcpListener;

#[derive(Clone)]
struct MockState {
    login_count: Arc<AtomicUsize>,
    spaces_count: Arc<AtomicUsize>,
    latest_token: Arc<tokio::sync::Mutex<String>>,
}

#[tokio::test]
async fn login_extracts_auth_token_and_persists_session() -> Result<()> {
    unsafe {
        std::env::set_var("DOCMOST_DISABLE_KEYRING", "1");
    }

    let server = spawn_mock_docmost().await?;
    let temp_dir = TempDir::new()?;
    let auth_manager = AuthManager::new(
        StartupConfig {
            base_url: Some(server.base_url.clone()),
        },
        Some(temp_dir.path().to_path_buf()),
    )?;

    let session = auth_manager
        .login(LoginInput {
            base_url: server.base_url.clone(),
            email: "jane@example.com".to_string(),
            password: "super-secret".to_string(),
        })
        .await?;

    assert_eq!(session.base_url, server.base_url);
    assert_eq!(session.email, "jane@example.com");
    assert!(session.token.starts_with("token-"));
    assert!(session.expires_at.is_some());

    let store = StateStore::new(Some(temp_dir.path().to_path_buf()))?;
    let stored_config = store
        .read_config()
        .await?
        .expect("config should be persisted");
    let stored_session = store
        .read_session()
        .await?
        .expect("session should be persisted");
    let stored_credentials = store
        .read_credentials()
        .await?
        .expect("credentials should be persisted");

    assert_eq!(stored_config.base_url, server.base_url);
    assert_eq!(stored_session.token, session.token);
    assert_eq!(stored_credentials.email, "jane@example.com");
    assert_eq!(server.state.login_count.load(Ordering::SeqCst), 1);

    server.shutdown.abort();
    Ok(())
}

#[tokio::test]
async fn docmost_client_retries_after_401() -> Result<()> {
    unsafe {
        std::env::set_var("DOCMOST_DISABLE_KEYRING", "1");
    }

    let server = spawn_mock_docmost().await?;
    let temp_dir = TempDir::new()?;
    let store = StateStore::new(Some(temp_dir.path().to_path_buf()))?;

    store
        .write_config(&StoredConfig {
            base_url: server.base_url.clone(),
            email: "jane@example.com".to_string(),
            last_authenticated_at: "2026-03-12T00:00:00.000Z".to_string(),
        })
        .await?;
    store
        .write_session(&StoredSession {
            token: "stale-token".to_string(),
            expires_at: Some(make_jwt_expiry(3600)),
            saved_at: "2026-03-12T00:00:00.000Z".to_string(),
        })
        .await?;
    store
        .write_credentials(&StoredCredentials {
            email: "jane@example.com".to_string(),
            password: "super-secret".to_string(),
        })
        .await?;

    let auth_manager = AuthManager::new(
        StartupConfig {
            base_url: Some(server.base_url.clone()),
        },
        Some(temp_dir.path().to_path_buf()),
    )?;
    let client = DocmostClient::new(auth_manager);

    let spaces = client.list_spaces().await?;

    assert_eq!(spaces.len(), 1);
    assert_eq!(spaces[0].name.as_deref(), Some("Engineering"));
    assert_eq!(server.state.login_count.load(Ordering::SeqCst), 1);
    assert_eq!(server.state.spaces_count.load(Ordering::SeqCst), 2);

    server.shutdown.abort();
    Ok(())
}

#[test]
fn extracts_auth_token_from_set_cookie_headers() {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.append(
        SET_COOKIE,
        HeaderValue::from_static("authToken=abc123%2E456; Path=/; HttpOnly"),
    );
    assert_eq!(
        read_auth_token_from_headers(&headers).as_deref(),
        Some("abc123.456")
    );
}

#[test]
fn decodes_jwt_expiry_without_verifying_signature() {
    let token = make_jwt_with_exp(1_900_000_000);
    let expires_at = get_jwt_expiry_iso(&token).expect("expiry should be parsed");
    assert!(expires_at.starts_with("2030-03"));
}

struct MockDocmostServer {
    base_url: String,
    state: MockState,
    shutdown: tokio::task::JoinHandle<()>,
}

async fn spawn_mock_docmost() -> Result<MockDocmostServer> {
    let login_count = Arc::new(AtomicUsize::new(0));
    let spaces_count = Arc::new(AtomicUsize::new(0));
    let latest_token = Arc::new(tokio::sync::Mutex::new(String::new()));

    let state = MockState {
        login_count,
        spaces_count,
        latest_token,
    };

    let app = Router::new()
        .route("/api/auth/login", post(login_route))
        .route("/api/spaces", post(spaces_route))
        .with_state(state.clone());
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let address = listener.local_addr()?;
    let shutdown = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    Ok(MockDocmostServer {
        base_url: normalize_base_url(&format!("http://{}", address)),
        state,
        shutdown,
    })
}

async fn login_route(State(state): State<MockState>) -> impl IntoResponse {
    let number = state.login_count.fetch_add(1, Ordering::SeqCst) + 1;
    let token = format!("token-{number}.{}.sig", payload_segment(3_000_000_000));
    *state.latest_token.lock().await = token.clone();

    let mut response = Json(serde_json::json!({ "ok": true })).into_response();
    response.headers_mut().append(
        SET_COOKIE,
        HeaderValue::from_str(&format!("authToken={token}; Path=/; HttpOnly")).unwrap(),
    );
    response
}

async fn spaces_route(State(state): State<MockState>, request: Request) -> impl IntoResponse {
    let count = state.spaces_count.fetch_add(1, Ordering::SeqCst) + 1;
    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let expected = format!("Bearer {}", state.latest_token.lock().await.clone());

    if count == 1 || auth_header != expected {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "unauthorized" })),
        )
            .into_response();
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "data": {
                "items": [{
                    "id": "space-1",
                    "name": "Engineering",
                    "slug": "engineering",
                    "description": "Internal docs",
                    "memberCount": 7
                }]
            }
        })),
    )
        .into_response()
}

fn make_jwt_expiry(offset_seconds: i64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs() as i64;
    make_jwt_with_exp(now + offset_seconds)
}

fn make_jwt_with_exp(exp: i64) -> String {
    format!("header.{}.signature", payload_segment(exp))
}

fn payload_segment(exp: i64) -> String {
    URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&serde_json::json!({
            "exp": exp
        }))
        .unwrap(),
    )
}
