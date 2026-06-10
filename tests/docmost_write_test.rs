use std::sync::{Arc, Mutex};

use anyhow::Result;
use axum::{Json, Router, body::Bytes, extract::State, http::HeaderMap, routing::post};
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
    json_bodies: Arc<Mutex<Vec<(String, Value)>>>,
    /// (route, raw multipart body as UTF-8) for the import endpoint.
    multipart_bodies: Arc<Mutex<Vec<(String, String)>>>,
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
        .route("/api/pages/import", post(import_route))
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
        .json_bodies
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
        .json_bodies
        .lock()
        .unwrap()
        .push(("update".to_string(), body));
    Json(json!({
        "data": { "id": "page-1", "slugId": "slug-1", "title": "Updated" },
        "success": true,
        "status": 200
    }))
}

/// Captures the raw multipart body so tests can assert the spaceId field and the
/// uploaded markdown file content.
async fn import_route(
    State(state): State<CapturedState>,
    headers: HeaderMap,
    body: Bytes,
) -> Json<Value> {
    let content_type = headers
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.starts_with("multipart/form-data"),
        "import must be multipart, got: {content_type}"
    );
    let raw = String::from_utf8_lossy(&body).to_string();
    state
        .multipart_bodies
        .lock()
        .unwrap()
        .push(("import".to_string(), raw));
    Json(json!({
        "data": { "id": "imported-1", "slugId": "islug-1", "title": "Imported", "spaceId": "space-1" },
        "success": true,
        "status": 200
    }))
}

fn last_json(state: &CapturedState, route: &str) -> Value {
    state
        .json_bodies
        .lock()
        .unwrap()
        .iter()
        .rev()
        .find(|(name, _)| name == route)
        .map(|(_, body)| body.clone())
        .unwrap_or_else(|| panic!("no recorded {route} JSON request"))
}

fn last_multipart(state: &CapturedState, route: &str) -> String {
    state
        .multipart_bodies
        .lock()
        .unwrap()
        .iter()
        .rev()
        .find(|(name, _)| name == route)
        .map(|(_, body)| body.clone())
        .unwrap_or_else(|| panic!("no recorded {route} multipart request"))
}

#[tokio::test]
async fn create_page_with_body_uses_import_with_markdown_file() -> Result<()> {
    let temp = TempDir::new()?;
    let (client, state, _) = spawn(&temp).await?;

    let page = client
        .create_page("space-1", "My Page", Some("Hello **world**"), None)
        .await?;
    // Response comes from the import endpoint.
    assert_eq!(page.id.as_deref(), Some("imported-1"));
    assert_eq!(page.slug_id.as_deref(), Some("islug-1"));

    let raw = last_multipart(&state, "import");
    // spaceId text field is present.
    assert!(
        raw.contains("name=\"spaceId\"") && raw.contains("space-1"),
        "missing spaceId field in:\n{raw}"
    );
    // A .md file part is uploaded.
    assert!(
        raw.contains("name=\"file\"") && raw.contains("filename=\"page.md\""),
        "missing .md file part in:\n{raw}"
    );
    // The title is prepended as a level-1 heading so the importer uses it as the title.
    assert!(
        raw.contains("# My Page"),
        "title heading missing in:\n{raw}"
    );
    // The body markdown is included verbatim (not pre-converted to ProseMirror JSON).
    assert!(raw.contains("Hello **world**"), "body missing in:\n{raw}");
    assert!(
        !raw.contains("\"type\":\"doc\""),
        "body must be raw markdown, not ProseMirror JSON, in:\n{raw}"
    );
    Ok(())
}

#[tokio::test]
async fn create_page_title_only_uses_create_endpoint() -> Result<()> {
    let temp = TempDir::new()?;
    let (client, state, _) = spawn(&temp).await?;

    // No body -> plain create endpoint, parent honored.
    client
        .create_page("space-1", "Title only", None, Some("parent-1"))
        .await?;

    let body = last_json(&state, "create");
    assert_eq!(body["spaceId"], json!("space-1"));
    assert_eq!(body["title"], json!("Title only"));
    assert_eq!(body["parentPageId"], json!("parent-1"));
    assert!(
        body.get("content").is_none(),
        "title-only create must not send content"
    );
    // And it must NOT have hit the import endpoint.
    assert!(
        state
            .multipart_bodies
            .lock()
            .unwrap()
            .iter()
            .all(|(name, _)| name != "import"),
        "title-only create should not use import"
    );
    Ok(())
}

#[tokio::test]
async fn create_page_blank_markdown_falls_back_to_title_only() -> Result<()> {
    let temp = TempDir::new()?;
    let (client, state, _) = spawn(&temp).await?;

    // Whitespace-only markdown is treated as "no body".
    client
        .create_page("space-1", "Blanky", Some("   \n  "), None)
        .await?;

    let body = last_json(&state, "create");
    assert_eq!(body["title"], json!("Blanky"));
    assert!(
        state.multipart_bodies.lock().unwrap().is_empty(),
        "blank markdown should not trigger import"
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

    let body = last_json(&state, "update");
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

    let body = last_json(&state, "update");
    assert_eq!(body["pageId"], json!("page-1"));
    assert_eq!(body["title"], json!("Renamed"));
    assert!(body.get("content").is_none());
    assert!(body.get("operation").is_none());
    assert!(body.get("format").is_none());
    Ok(())
}
