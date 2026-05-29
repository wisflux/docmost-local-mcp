use std::sync::{Arc, Mutex};

use anyhow::Result;
use axum::{Json, Router, extract::State, routing::post};
use docmost_local_mcp::{
    docmost_client::DocmostClient,
    prosemirror::markdown_to_prosemirror,
    startup_config::normalize_base_url,
    storage::state_store::StateStore,
    types::{AuthenticatedSession, StartupConfig, StoredConfig, StoredSession},
};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::net::TcpListener;

#[derive(Clone, Default)]
struct CapturedState {
    bodies: Arc<Mutex<Vec<(String, Value)>>>,
}

/// Spin up a mock Docmost that records request bodies and returns a page envelope.
/// The auth manager is pre-seeded with a far-future session so no login occurs.
async fn spawn(temp: &TempDir) -> Result<(DocmostClient, CapturedState, String)> {
    unsafe {
        std::env::set_var("DOCMOST_DISABLE_KEYRING", "1");
    }

    let state = CapturedState::default();
    let app = Router::new()
        .route("/api/pages/create", post(create_route))
        .route("/api/pages/update", post(update_route))
        .with_state(state.clone());
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let address = listener.local_addr()?;
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    let base_url = normalize_base_url(&format!("http://{address}"));

    // Pre-seed a valid (non-expiring) session so the client skips interactive auth.
    let store = StateStore::new(Some(temp.path().to_path_buf()))?;
    store
        .write_config(&StoredConfig {
            base_url: base_url.clone(),
            email: "jane@example.com".to_string(),
            last_authenticated_at: "2026-03-12T00:00:00.000Z".to_string(),
        })
        .await?;
    store
        .write_session(&StoredSession {
            // No expiry => never treated as expiring.
            token: "seed-token".to_string(),
            expires_at: None,
            saved_at: "2026-03-12T00:00:00.000Z".to_string(),
        })
        .await?;

    let auth_manager = docmost_local_mcp::auth::manager::AuthManager::new(
        StartupConfig {
            base_url: Some(base_url.clone()),
        },
        Some(temp.path().to_path_buf()),
    )?;
    // Sanity: the seeded session resolves without hitting the network.
    let session: AuthenticatedSession = auth_manager.get_authenticated_session().await?;
    assert_eq!(session.token, "seed-token");

    Ok((DocmostClient::new(auth_manager), state, base_url))
}

async fn create_route(State(state): State<CapturedState>, Json(body): Json<Value>) -> Json<Value> {
    state
        .bodies
        .lock()
        .unwrap()
        .push(("create".to_string(), body));
    Json(json!({
        "data": { "id": "page-1", "slugId": "slug-1", "title": "Created", "spaceId": "space-1" },
        "success": true,
        "status": 200
    }))
}

async fn update_route(State(state): State<CapturedState>, Json(body): Json<Value>) -> Json<Value> {
    state
        .bodies
        .lock()
        .unwrap()
        .push(("update".to_string(), body));
    Json(json!({
        "data": { "id": "page-1", "slugId": "slug-1", "title": "Updated" },
        "success": true,
        "status": 200
    }))
}

fn last_body(state: &CapturedState, route: &str) -> Value {
    state
        .bodies
        .lock()
        .unwrap()
        .iter()
        .rev()
        .find(|(name, _)| name == route)
        .map(|(_, body)| body.clone())
        .unwrap_or_else(|| panic!("no recorded {route} request"))
}

#[tokio::test]
async fn create_page_sends_content_object_and_parent() -> Result<()> {
    let temp = TempDir::new()?;
    let (client, state, _) = spawn(&temp).await?;

    let content = markdown_to_prosemirror("# Hello\n\nbody");
    let page = client
        .create_page("space-1", "My Page", Some(&content), Some("parent-1"))
        .await?;
    assert_eq!(page.id.as_deref(), Some("page-1"));
    assert_eq!(page.slug_id.as_deref(), Some("slug-1"));

    let body = last_body(&state, "create");
    assert_eq!(body["spaceId"], json!("space-1"));
    assert_eq!(body["title"], json!("My Page"));
    assert_eq!(body["parentPageId"], json!("parent-1"));
    // content must be an OBJECT (ProseMirror doc), never a stringified JSON.
    assert!(body["content"].is_object());
    assert_eq!(body["content"]["type"], json!("doc"));
    Ok(())
}

#[tokio::test]
async fn create_page_omits_optional_fields_when_absent() -> Result<()> {
    let temp = TempDir::new()?;
    let (client, state, _) = spawn(&temp).await?;

    client
        .create_page("space-1", "Title only", None, None)
        .await?;

    let body = last_body(&state, "create");
    assert_eq!(body["spaceId"], json!("space-1"));
    assert_eq!(body["title"], json!("Title only"));
    assert!(body.get("content").is_none(), "content must be omitted");
    assert!(
        body.get("parentPageId").is_none(),
        "parentPageId must be omitted"
    );
    Ok(())
}

#[tokio::test]
async fn update_page_sends_operation_and_format_with_content() -> Result<()> {
    let temp = TempDir::new()?;
    let (client, state, _) = spawn(&temp).await?;

    let content = markdown_to_prosemirror("new body");
    client
        .update_page("page-1", Some("New Title"), Some(&content))
        .await?;

    let body = last_body(&state, "update");
    assert_eq!(body["pageId"], json!("page-1"));
    assert_eq!(body["title"], json!("New Title"));
    assert!(body["content"].is_object());
    // The critical, easy-to-forget bit: Docmost ignores content unless BOTH
    // operation and format accompany it.
    assert_eq!(body["operation"], json!("replace"));
    assert_eq!(body["format"], json!("json"));
    Ok(())
}

#[tokio::test]
async fn update_page_title_only_omits_content_operation_format() -> Result<()> {
    let temp = TempDir::new()?;
    let (client, state, _) = spawn(&temp).await?;

    client.update_page("page-1", Some("Renamed"), None).await?;

    let body = last_body(&state, "update");
    assert_eq!(body["pageId"], json!("page-1"));
    assert_eq!(body["title"], json!("Renamed"));
    assert!(body.get("content").is_none());
    assert!(body.get("operation").is_none());
    assert!(body.get("format").is_none());
    Ok(())
}
