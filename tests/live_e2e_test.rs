//! Live end-to-end test against a REAL Docmost instance.
//!
//! This is `#[ignore]`d so it never runs in CI or a normal `cargo test` — it talks to
//! a real server and creates a real page. Run it explicitly with credentials in the
//! environment (see `scripts/e2e-live.sh` or `docs/write-tools.md`):
//!
//! ```bash
//! DOCMOST_BASE_URL=https://docs.example.com \
//! DOCMOST_EMAIL=you@example.com \
//! DOCMOST_PASSWORD=... \
//! cargo test --test live_e2e_test --no-default-features -- --ignored --nocapture
//! ```
//!
//! Optional env:
//! - `DOCMOST_SPACE_ID`   pick a space explicitly (otherwise the first from list_spaces)
//! - `DOCMOST_PARENT_PAGE_ID`  create the page under this parent
//!
//! The test always leaves the created page in place (this server has no delete tool
//! yet — the page is a throwaway you can remove from the UI).
//!
//! Credentials are read from the environment at run time and are never written to the
//! repo. The session is cached under a temp dir, not your real `~/.docmost-local-mcp`.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use docmost_local_mcp::{
    auth::manager::AuthManager,
    docmost_client::DocmostClient,
    prosemirror::prosemirror_to_markdown,
    startup_config::normalize_base_url,
    types::{LoginInput, StartupConfig},
};
use tempfile::TempDir;
use tokio::time::sleep;

fn env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

/// A representative document exercising most of the converter's node/mark set.
/// Note: no leading `# heading` — the tool prepends `# {title}` itself, and the
/// importer turns the first H1 into the page title (removing it from the body).
const SAMPLE_MARKDOWN: &str = "\
A paragraph with **bold**, *italic*, `inline code`, ~~strike~~, and a \
[link](https://docmost.com).

- bullet one
- bullet two
  - nested bullet

1. first
2. second

- [ ] open task
- [x] done task

```rust
fn main() { println!(\"hello\"); }
```

> a blockquote

| Col A | Col B |
| --- | --- |
| 1 | 2 |
";

#[tokio::test]
#[ignore = "live: needs a real Docmost (set DOCMOST_BASE_URL/EMAIL/PASSWORD), creates a real page"]
async fn live_create_get_update_roundtrip() -> Result<()> {
    // Use the encrypted-file credential fallback in a temp dir; never the OS keyring
    // or the user's real ~/.docmost-local-mcp.
    unsafe {
        std::env::set_var("DOCMOST_DISABLE_KEYRING", "1");
    }

    let base_url = normalize_base_url(
        &env("DOCMOST_BASE_URL").context("DOCMOST_BASE_URL must be set for the live E2E test")?,
    );
    let email = env("DOCMOST_EMAIL").context("DOCMOST_EMAIL must be set")?;
    let password = env("DOCMOST_PASSWORD").context("DOCMOST_PASSWORD must be set")?;

    let temp = TempDir::new()?;
    let auth_manager = AuthManager::new(
        StartupConfig {
            base_url: Some(base_url.clone()),
        },
        Some(temp.path().to_path_buf()),
    )?;

    // --- Phase 2: headless login (no interactive browser) ---
    eprintln!("[e2e] logging in to {base_url} as {email} ...");
    auth_manager
        .login(LoginInput {
            base_url: base_url.clone(),
            email: email.clone(),
            password,
        })
        .await
        .context("headless login failed (check base URL / credentials)")?;
    eprintln!("[e2e] login ok");

    let client = DocmostClient::new(auth_manager);

    // --- Phase 3.1: choose a space ---
    let space_id = match env("DOCMOST_SPACE_ID") {
        Some(space_id) => space_id,
        None => {
            let spaces = client.list_spaces().await.context("list_spaces failed")?;
            let space = spaces
                .first()
                .context("no spaces available; set DOCMOST_SPACE_ID")?;
            eprintln!(
                "[e2e] using first space: {} ({})",
                space.name.as_deref().unwrap_or("Untitled"),
                space.id
            );
            space.id.clone()
        }
    };
    let parent_page_id = env("DOCMOST_PARENT_PAGE_ID");

    // --- Phase 3.2: create_page (body routed through the import endpoint) ---
    let title = "E2E Test Page (docmost-local-mcp)";
    let created = client
        .create_page(
            &space_id,
            title,
            Some(SAMPLE_MARKDOWN),
            parent_page_id.as_deref(),
        )
        .await
        .context("create_page failed")?;

    let page_id = created
        .id
        .clone()
        .context("create response missing page id")?;
    let slug_id = created
        .slug_id
        .clone()
        .context("create response missing slug id")?;
    eprintln!(
        "[e2e] created page id={page_id} slug={slug_id} title={:?}",
        created.title
    );

    // The prepended `# {title}` becomes the page title (importer strips it from the body).
    if created.title.as_deref() != Some(title) {
        bail!(
            "expected imported page title {title:?}, got {:?}",
            created.title
        );
    }

    // --- Phase 3.3: get_page confirms the BODY actually persisted ---
    let fetched = client
        .get_page(&slug_id)
        .await
        .context("get_page after create failed")?
        .context("get_page returned no page after create")?;
    let rendered = fetched
        .content
        .as_ref()
        .map(prosemirror_to_markdown)
        .unwrap_or_default();
    eprintln!("[e2e] fetched page markdown after create:\n{rendered}\n");

    // The body content (everything after the title heading) must be present.
    for needle in [
        "**bold**",
        "[link](https://docmost.com)",
        "- [ ] open task",
        "- [x] done task",
        "```rust",
        "| Col A | Col B |",
    ] {
        if !rendered.contains(needle) {
            bail!("created page is missing expected content {needle:?}\n--- got ---\n{rendered}");
        }
    }
    eprintln!("[e2e] create body persisted OK");

    // --- Phase 3.4: update_page TITLE (body update is not supported on Docmost <= 0.25.x) ---
    let updated_title = "E2E Test Page (updated title)";
    client
        .update_page(&page_id, Some(updated_title), None)
        .await
        .context("update_page (title) failed")?;
    eprintln!("[e2e] update_page title sent; polling get_page for persistence ...");

    let mut last_seen = String::new();
    let mut persisted = false;
    for attempt in 1..=10 {
        sleep(Duration::from_millis(1000)).await;
        let page = client
            .get_page(&slug_id)
            .await
            .context("get_page during update poll failed")?
            .context("get_page returned no page during update poll")?;
        last_seen = page
            .content
            .as_ref()
            .map(prosemirror_to_markdown)
            .unwrap_or_default();
        let title_ok = page.title.as_deref() == Some(updated_title);
        // The body must survive a title-only update unchanged.
        let body_ok = last_seen.contains("**bold**") && last_seen.contains("| Col A | Col B |");
        eprintln!("[e2e] poll {attempt}/10: title_ok={title_ok} body_ok={body_ok}");
        if title_ok && body_ok {
            persisted = true;
            break;
        }
    }

    if !persisted {
        bail!(
            "title update did not persist (or body was lost) within the poll window.\n\
             --- last seen markdown ---\n{last_seen}"
        );
    }

    eprintln!("[e2e] title update persisted and body preserved OK");
    eprintln!(
        "[e2e] PASS. Created/updated page slug={slug_id} (id={page_id}) in space {space_id}. \
         No delete tool exists yet, so the page remains — remove it from the Docmost UI if desired."
    );
    Ok(())
}
