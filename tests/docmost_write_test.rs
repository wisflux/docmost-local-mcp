use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU16, AtomicUsize, Ordering},
};

use anyhow::Result;
use axum::{
    Json, Router,
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode, header::SET_COOKIE},
    routing::post,
};
use docmost_local_mcp::{
    docmost_client::DocmostClient,
    prosemirror::markdown_to_prosemirror,
    startup_config::normalize_base_url,
    storage::state_store::StateStore,
    types::{AuthenticatedSession, StartupConfig, StoredConfig, StoredCredentials, StoredSession},
};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::net::TcpListener;

#[derive(Clone, Default)]
struct CapturedState {
    json_bodies: Arc<Mutex<Vec<(String, Value)>>>,
    /// (route, raw multipart body as UTF-8) for the import endpoint.
    multipart_bodies: Arc<Mutex<Vec<(String, String)>>>,
    /// When true, the first hit of each write route returns 401 once (retry testing).
    fail_first_401: Arc<AtomicBool>,
    /// When non-zero, the next write route returns this HTTP status once (error testing).
    force_status: Arc<AtomicU16>,
    create_attempts: Arc<AtomicUsize>,
    update_attempts: Arc<AtomicUsize>,
    import_attempts: Arc<AtomicUsize>,
    /// How many times the mock `/api/auth/login` endpoint was called.
    login_hits: Arc<AtomicUsize>,
}

/// Returns an injected failure status for this attempt, if one is configured. Consumes a
/// single forced status (once) and, when armed, fails the first attempt of a route with 401.
fn injected_status(state: &CapturedState, attempts: &Arc<AtomicUsize>) -> Option<StatusCode> {
    let forced = state.force_status.swap(0, Ordering::SeqCst);
    if forced != 0 {
        return Some(StatusCode::from_u16(forced).expect("valid status"));
    }
    if state.fail_first_401.load(Ordering::SeqCst) && attempts.fetch_add(1, Ordering::SeqCst) == 0 {
        return Some(StatusCode::UNAUTHORIZED);
    }
    None
}

/// Spin up a mock Docmost that records request bodies and returns a page envelope.
/// The auth manager is pre-seeded with a far-future session so no login occurs, plus
/// saved credentials so a forced 401 can be recovered via the mock login endpoint.
async fn spawn(temp: &TempDir) -> Result<(DocmostClient, CapturedState, String)> {
    unsafe {
        std::env::set_var("DOCMOST_DISABLE_KEYRING", "1");
    }

    let state = CapturedState::default();
    let app = Router::new()
        .route("/api/pages/create", post(create_route))
        .route("/api/pages/update", post(update_route))
        .route("/api/pages/import", post(import_route))
        .route("/api/auth/login", post(login_route))
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
    // Saved credentials let reauthenticate() recover a 401 headlessly via the mock login.
    store
        .write_credentials(&StoredCredentials {
            email: "jane@example.com".to_string(),
            password: "secret".to_string(),
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

async fn create_route(
    State(state): State<CapturedState>,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    if let Some(status) = injected_status(&state, &state.create_attempts) {
        return (status, Json(json!({ "message": "injected failure" })));
    }
    state
        .json_bodies
        .lock()
        .unwrap()
        .push(("create".to_string(), body));
    (
        StatusCode::OK,
        Json(json!({
            "data": { "id": "page-1", "slugId": "slug-1", "title": "Created", "spaceId": "space-1" },
            "success": true,
            "status": 200
        })),
    )
}

async fn update_route(
    State(state): State<CapturedState>,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    if let Some(status) = injected_status(&state, &state.update_attempts) {
        return (status, Json(json!({ "message": "injected failure" })));
    }
    state
        .json_bodies
        .lock()
        .unwrap()
        .push(("update".to_string(), body));
    (
        StatusCode::OK,
        Json(json!({
            "data": { "id": "page-1", "slugId": "slug-1", "title": "Updated" },
            "success": true,
            "status": 200
        })),
    )
}

/// Captures the raw multipart body so tests can assert the spaceId field and the
/// uploaded markdown file content.
async fn import_route(
    State(state): State<CapturedState>,
    headers: HeaderMap,
    body: Bytes,
) -> (StatusCode, Json<Value>) {
    if let Some(status) = injected_status(&state, &state.import_attempts) {
        return (status, Json(json!({ "message": "injected failure" })));
    }
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
    (
        StatusCode::OK,
        Json(json!({
            "data": { "id": "imported-1", "slugId": "islug-1", "title": "Imported", "spaceId": "space-1" },
            "success": true,
            "status": 200
        })),
    )
}

/// Mock Docmost login: returns the auth token via a Set-Cookie header, matching the real
/// server contract the auth manager parses.
async fn login_route(
    State(state): State<CapturedState>,
) -> (
    StatusCode,
    [(axum::http::HeaderName, &'static str); 1],
    Json<Value>,
) {
    state.login_hits.fetch_add(1, Ordering::SeqCst);
    (
        StatusCode::OK,
        [(SET_COOKIE, "authToken=reauth-token-123; Path=/; HttpOnly")],
        Json(json!({ "data": { "userId": "user-1" }, "success": true })),
    )
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
async fn create_page_title_only_without_parent_omits_parent_field() -> Result<()> {
    let temp = TempDir::new()?;
    let (client, state, _) = spawn(&temp).await?;

    client
        .create_page("space-1", "Root Page", None, None)
        .await?;

    let body = last_json(&state, "create");
    assert_eq!(body["spaceId"], json!("space-1"));
    assert_eq!(body["title"], json!("Root Page"));
    assert!(
        body.get("parentPageId").is_none(),
        "no parent supplied => parentPageId must be omitted, got: {body}"
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
async fn create_page_with_body_ignores_parent_page_id() -> Result<()> {
    // Characterization test for a documented limitation: the import endpoint has no
    // parent parameter, so a body-bearing create silently lands at the space root and
    // `parent_page_id` is dropped. If this ever changes, update the tool docs/response.
    let temp = TempDir::new()?;
    let (client, state, _) = spawn(&temp).await?;

    client
        .create_page(
            "space-1",
            "Child?",
            Some("body text"),
            Some("parent-should-be-ignored"),
        )
        .await?;

    // It went through import (not create), and the parent id is nowhere in the request.
    let raw = last_multipart(&state, "import");
    assert!(
        !raw.contains("parent-should-be-ignored"),
        "parent id must not leak into the import request:\n{raw}"
    );
    assert!(
        state.json_bodies.lock().unwrap().is_empty(),
        "body create must not hit the plain create endpoint"
    );
    Ok(())
}

#[tokio::test]
async fn import_prepends_title_and_preserves_multiline_body() -> Result<()> {
    let temp = TempDir::new()?;
    let (client, state, _) = spawn(&temp).await?;

    let body = "## Section\n\nLine one.\n\n- a\n- b";
    client
        .create_page("space-1", "Doc Title", Some(body), None)
        .await?;

    let raw = last_multipart(&state, "import");
    // Title prepended as an H1 ahead of the original body, which is preserved verbatim.
    let file_start = raw
        .find("# Doc Title")
        .expect("prepended title heading present");
    for needle in ["## Section", "Line one.", "- a", "- b"] {
        let at = raw
            .find(needle)
            .unwrap_or_else(|| panic!("missing {needle} in:\n{raw}"));
        assert!(
            at > file_start,
            "body must follow the title heading: {needle}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn create_page_retries_import_on_unauthorized() -> Result<()> {
    let temp = TempDir::new()?;
    let (client, state, _) = spawn(&temp).await?;
    state.fail_first_401.store(true, Ordering::SeqCst);

    // First import attempt returns 401 -> reauth via login -> second attempt succeeds.
    let page = client
        .create_page("space-1", "Retry Import", Some("body"), None)
        .await?;

    assert_eq!(page.id.as_deref(), Some("imported-1"));
    assert_eq!(
        state.login_hits.load(Ordering::SeqCst),
        1,
        "should have reauthenticated exactly once"
    );
    assert_eq!(
        state.import_attempts.load(Ordering::SeqCst),
        2,
        "import should be attempted twice (401 then success)"
    );
    // Exactly one successful import body was recorded (the retry).
    assert_eq!(state.multipart_bodies.lock().unwrap().len(), 1);
    Ok(())
}

#[tokio::test]
async fn update_page_retries_on_unauthorized() -> Result<()> {
    let temp = TempDir::new()?;
    let (client, state, _) = spawn(&temp).await?;
    state.fail_first_401.store(true, Ordering::SeqCst);

    let page = client.update_page("page-1", Some("Renamed"), None).await?;

    assert_eq!(page.id.as_deref(), Some("page-1"));
    assert_eq!(state.login_hits.load(Ordering::SeqCst), 1);
    let body = last_json(&state, "update");
    assert_eq!(body["title"], json!("Renamed"));
    Ok(())
}

#[tokio::test]
async fn create_page_surfaces_server_error_without_panicking() -> Result<()> {
    let temp = TempDir::new()?;
    let (client, state, _) = spawn(&temp).await?;
    // Simulate a permission failure on the plain create endpoint.
    state.force_status.store(403, Ordering::SeqCst);

    let error = client
        .create_page("space-1", "Forbidden", None, None)
        .await
        .expect_err("403 must surface as an error");
    assert!(
        error.to_string().contains("403"),
        "error should mention the status: {error}"
    );
    Ok(())
}

#[tokio::test]
async fn import_surfaces_server_error_without_panicking() -> Result<()> {
    let temp = TempDir::new()?;
    let (client, state, _) = spawn(&temp).await?;
    state.force_status.store(403, Ordering::SeqCst);

    let error = client
        .create_page("space-1", "Forbidden", Some("body"), None)
        .await
        .expect_err("403 on import must surface as an error");
    assert!(
        error.to_string().contains("403"),
        "error should mention the status: {error}"
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

#[tokio::test]
async fn update_page_with_no_changes_sends_only_page_id() -> Result<()> {
    // Neither a title nor a body: the payload carries just the page id.
    let temp = TempDir::new()?;
    let (client, state, _) = spawn(&temp).await?;

    client.update_page("page-9", None, None).await?;

    let body = last_json(&state, "update");
    assert_eq!(body["pageId"], json!("page-9"));
    assert!(body.get("title").is_none());
    assert!(body.get("content").is_none());
    Ok(())
}
