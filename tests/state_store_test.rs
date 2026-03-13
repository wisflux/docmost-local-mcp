use docmost_local_mcp::{
    storage::state_store::StateStore,
    types::{StoredConfig, StoredCredentials, StoredSession},
};
use tempfile::TempDir;

#[tokio::test]
async fn persists_config_session_and_encrypted_credentials() {
    unsafe {
        std::env::set_var("DOCMOST_DISABLE_KEYRING", "1");
    }
    let temp_dir = TempDir::new().unwrap();
    let store = StateStore::new(Some(temp_dir.path().to_path_buf())).unwrap();

    store
        .write_config(&StoredConfig {
            base_url: "https://docs.example.com".to_string(),
            email: "jane@example.com".to_string(),
            last_authenticated_at: "2026-03-12T00:00:00.000Z".to_string(),
        })
        .await
        .unwrap();
    store
        .write_session(&StoredSession {
            token: "token-value".to_string(),
            expires_at: Some("2026-03-12T01:00:00.000Z".to_string()),
            saved_at: "2026-03-12T00:00:00.000Z".to_string(),
        })
        .await
        .unwrap();
    store
        .write_credentials(&StoredCredentials {
            email: "jane@example.com".to_string(),
            password: "super-secret".to_string(),
        })
        .await
        .unwrap();

    assert_eq!(
        store.read_config().await.unwrap(),
        Some(StoredConfig {
            base_url: "https://docs.example.com".to_string(),
            email: "jane@example.com".to_string(),
            last_authenticated_at: "2026-03-12T00:00:00.000Z".to_string(),
        })
    );
    assert_eq!(
        store.read_session().await.unwrap(),
        Some(StoredSession {
            token: "token-value".to_string(),
            expires_at: Some("2026-03-12T01:00:00.000Z".to_string()),
            saved_at: "2026-03-12T00:00:00.000Z".to_string(),
        })
    );
    assert_eq!(
        store.read_credentials().await.unwrap(),
        Some(StoredCredentials {
            email: "jane@example.com".to_string(),
            password: "super-secret".to_string(),
        })
    );
}

#[tokio::test]
async fn clears_saved_session_without_touching_credentials() {
    unsafe {
        std::env::set_var("DOCMOST_DISABLE_KEYRING", "1");
    }
    let temp_dir = TempDir::new().unwrap();
    let store = StateStore::new(Some(temp_dir.path().to_path_buf())).unwrap();

    store
        .write_session(&StoredSession {
            token: "token-value".to_string(),
            expires_at: None,
            saved_at: "2026-03-12T00:00:00.000Z".to_string(),
        })
        .await
        .unwrap();
    store
        .write_credentials(&StoredCredentials {
            email: "jane@example.com".to_string(),
            password: "super-secret".to_string(),
        })
        .await
        .unwrap();

    store.clear_session().await.unwrap();

    assert_eq!(store.read_session().await.unwrap(), None);
    assert_eq!(
        store.read_credentials().await.unwrap(),
        Some(StoredCredentials {
            email: "jane@example.com".to_string(),
            password: "super-secret".to_string(),
        })
    );
}
