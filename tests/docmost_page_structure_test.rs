//! Client tests for the page-structure write methods (duplicate / copy-to-space /
//! move / move-to-space) against a mock Docmost that records request bodies.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use anyhow::Result;
use axum::{Json, Router, extract::State, routing::post};
use docmost_local_mcp::{
    docmost_client::DocmostClient,
    startup_config::normalize_base_url,
    storage::state_store::StateStore,
    types::{StartupConfig, StoredConfig, StoredSession},
};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::net::TcpListener;

#[derive(Clone, Default)]
struct Captured {
    bodies: Arc<Mutex<Vec<(String, Value)>>>,
    /// When set, the sidebar endpoint returns no siblings.
    empty_siblings: Arc<AtomicBool>,
}

const MOVED_PAGE_ID: &str = "page-1";

async fn spawn(temp: &TempDir) -> Result<(DocmostClient, Captured)> {
    unsafe {
        std::env::set_var("DOCMOST_DISABLE_KEYRING", "1");
    }
    let state = Captured::default();
    let app = Router::new()
        .route("/api/pages/info", post(info_route))
        .route("/api/pages/sidebar-pages", post(sidebar_route))
        .route("/api/pages/duplicate", post(duplicate_route))
        .route("/api/pages/move", post(move_route))
        .route("/api/pages/move-to-space", post(move_to_space_route))
        .route("/api/spaces/create", post(create_space_route))
        .route("/api/spaces/update", post(update_space_route))
        .with_state(state.clone());
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let address = listener.local_addr()?;
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    let base_url = normalize_base_url(&format!("http://{address}"));

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
    Ok((DocmostClient::new(auth_manager), state))
}

fn record(state: &Captured, route: &str, body: Value) {
    state.bodies.lock().unwrap().push((route.to_string(), body));
}

async fn info_route(State(state): State<Captured>, Json(body): Json<Value>) -> Json<Value> {
    record(&state, "info", body);
    Json(json!({
        "data": { "id": MOVED_PAGE_ID, "slugId": "slug-1", "title": "Doc", "spaceId": "space-1", "parentPageId": null },
        "success": true, "status": 200
    }))
}

async fn sidebar_route(State(state): State<Captured>, Json(body): Json<Value>) -> Json<Value> {
    record(&state, "sidebar", body);
    let items = if state.empty_siblings.load(Ordering::SeqCst) {
        json!([])
    } else {
        // Includes the moved page itself (position "a5") to prove it is excluded when
        // computing the append point — the generated position must land after "a1", the
        // last *other* sibling, and before "a5".
        json!([
            { "id": "c1", "slugId": "cs1", "position": "a0" },
            { "id": "c2", "slugId": "cs2", "position": "a1" },
            { "id": MOVED_PAGE_ID, "slugId": "slug-1", "position": "a5" },
        ])
    };
    Json(json!({ "data": { "items": items }, "success": true, "status": 200 }))
}

async fn duplicate_route(State(state): State<Captured>, Json(body): Json<Value>) -> Json<Value> {
    record(&state, "duplicate", body);
    Json(json!({
        "data": { "id": "dup-1", "slugId": "dslug-1", "title": "Doc (copy)", "spaceId": "space-1" },
        "success": true, "status": 200
    }))
}

async fn move_route(State(state): State<Captured>, Json(body): Json<Value>) -> Json<Value> {
    record(&state, "move", body);
    // The move endpoint returns the moved page; the client re-fetches via /info anyway.
    Json(json!({ "data": { "id": MOVED_PAGE_ID }, "success": true, "status": 200 }))
}

async fn move_to_space_route(
    State(state): State<Captured>,
    Json(body): Json<Value>,
) -> Json<Value> {
    record(&state, "move-to-space", body);
    // Docmost returns no data payload here — the client must tolerate that.
    Json(json!({ "success": true, "status": 200 }))
}

async fn create_space_route(State(state): State<Captured>, Json(body): Json<Value>) -> Json<Value> {
    record(&state, "space-create", body);
    Json(json!({
        "data": { "id": "space-new", "name": "New Space", "slug": "new-space" },
        "success": true, "status": 200
    }))
}

async fn update_space_route(State(state): State<Captured>, Json(body): Json<Value>) -> Json<Value> {
    record(&state, "space-update", body);
    Json(json!({
        "data": { "id": "space-1", "name": "Renamed", "slug": "renamed" },
        "success": true, "status": 200
    }))
}

fn last(state: &Captured, route: &str) -> Value {
    state
        .bodies
        .lock()
        .unwrap()
        .iter()
        .rev()
        .find(|(name, _)| name == route)
        .map(|(_, b)| b.clone())
        .unwrap_or_else(|| panic!("no recorded {route} request"))
}

#[tokio::test]
async fn duplicate_page_posts_page_id_only() -> Result<()> {
    let temp = TempDir::new()?;
    let (client, state) = spawn(&temp).await?;

    let page = client.duplicate_page("src-page", None).await?;
    assert_eq!(page.id.as_deref(), Some("dup-1"));

    let body = last(&state, "duplicate");
    assert_eq!(body["pageId"], json!("src-page"));
    assert!(
        body.get("spaceId").is_none(),
        "same-space duplicate must not send spaceId"
    );
    Ok(())
}

#[tokio::test]
async fn copy_page_to_space_sends_target_space() -> Result<()> {
    let temp = TempDir::new()?;
    let (client, state) = spawn(&temp).await?;

    client.duplicate_page("src-page", Some("space-9")).await?;

    let body = last(&state, "duplicate");
    assert_eq!(body["pageId"], json!("src-page"));
    assert_eq!(body["spaceId"], json!("space-9"));
    Ok(())
}

#[tokio::test]
async fn move_page_to_space_tolerates_empty_response() -> Result<()> {
    let temp = TempDir::new()?;
    let (client, state) = spawn(&temp).await?;

    // Returns () despite the endpoint sending no data payload.
    client.move_page_to_space("p1", "space-9").await?;

    let body = last(&state, "move-to-space");
    assert_eq!(body["pageId"], json!("p1"));
    assert_eq!(body["spaceId"], json!("space-9"));
    Ok(())
}

#[tokio::test]
async fn move_page_appends_after_last_sibling_excluding_itself() -> Result<()> {
    let temp = TempDir::new()?;
    let (client, state) = spawn(&temp).await?;

    client.move_page(MOVED_PAGE_ID, Some("parent-9")).await?;

    // Sibling lookup targets the parent within the page's space.
    let sidebar = last(&state, "sidebar");
    assert_eq!(sidebar["spaceId"], json!("space-1"));
    assert_eq!(sidebar["pageId"], json!("parent-9"));

    let mv = last(&state, "move");
    assert_eq!(mv["pageId"], json!(MOVED_PAGE_ID));
    assert_eq!(mv["parentPageId"], json!("parent-9"));
    let position = mv["position"].as_str().expect("position string");
    // Appended after "a1" (the last *other* sibling); "a5" belongs to the moved page and
    // is excluded, so the new position sorts strictly between "a1" and "a5".
    assert!(position > "a1", "position {position} must sort after a1");
    assert!(
        position < "a5",
        "position {position} must sort before excluded self a5"
    );
    assert!((5..=12).contains(&position.len()));
    Ok(())
}

#[tokio::test]
async fn move_page_to_root_omits_parent_and_queries_space_root() -> Result<()> {
    let temp = TempDir::new()?;
    let (client, state) = spawn(&temp).await?;

    client.move_page(MOVED_PAGE_ID, None).await?;

    let sidebar = last(&state, "sidebar");
    assert_eq!(sidebar["spaceId"], json!("space-1"));
    assert!(
        sidebar.get("pageId").is_none(),
        "root move must query the space root (no pageId)"
    );

    let mv = last(&state, "move");
    assert!(
        mv.get("parentPageId").is_none(),
        "root move omits parentPageId"
    );
    assert!(mv["position"].as_str().unwrap() > "a1");
    Ok(())
}

#[tokio::test]
async fn move_page_with_no_siblings_uses_start_position() -> Result<()> {
    let temp = TempDir::new()?;
    let (client, state) = spawn(&temp).await?;
    state.empty_siblings.store(true, Ordering::SeqCst);

    client
        .move_page(MOVED_PAGE_ID, Some("empty-parent"))
        .await?;

    let mv = last(&state, "move");
    let position = mv["position"].as_str().expect("position string");
    // First child => a start key (jittered "a0…"), still within the 5..=12 length rule.
    assert!(
        position.starts_with("a0"),
        "expected start key, got {position}"
    );
    assert!((5..=12).contains(&position.len()));
    Ok(())
}

#[tokio::test]
async fn create_space_sends_name_slug_and_description() -> Result<()> {
    let temp = TempDir::new()?;
    let (client, state) = spawn(&temp).await?;

    let space = client
        .create_space("New Space", "new-space", Some("A description"))
        .await?;
    assert_eq!(space.id, "space-new");

    let body = last(&state, "space-create");
    assert_eq!(body["name"], json!("New Space"));
    assert_eq!(body["slug"], json!("new-space"));
    assert_eq!(body["description"], json!("A description"));
    Ok(())
}

#[tokio::test]
async fn create_space_without_description_omits_it() -> Result<()> {
    let temp = TempDir::new()?;
    let (client, state) = spawn(&temp).await?;

    client.create_space("Docs", "docs", None).await?;

    let body = last(&state, "space-create");
    assert_eq!(body["slug"], json!("docs"));
    assert!(body.get("description").is_none());
    Ok(())
}

#[tokio::test]
async fn update_space_sends_only_provided_fields() -> Result<()> {
    let temp = TempDir::new()?;
    let (client, state) = spawn(&temp).await?;

    // Only the name changes; slug and description are left untouched.
    client
        .update_space("space-1", Some("Renamed"), None, None)
        .await?;

    let body = last(&state, "space-update");
    assert_eq!(body["spaceId"], json!("space-1"));
    assert_eq!(body["name"], json!("Renamed"));
    assert!(body.get("slug").is_none());
    assert!(body.get("description").is_none());
    Ok(())
}
