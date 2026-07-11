use anyhow::{Context, Result, anyhow};
use serde_json::Value;

use super::{CursorListResult, normalize_cursor_list_result};
use crate::{
    debug::debug_log,
    position::generate_jittered_key_between,
    types::{DocmostComment, DocmostPage, DocmostPageListItem, DocmostSpace},
};

/// Write operations (page create/update + structural moves). Split from the read
/// methods to keep each file within the size limit; all share `DocmostClient`'s
/// private request plumbing (accessible from this child module).
impl super::DocmostClient {
    /// Create a Docmost page.
    ///
    /// When `markdown` body content is supplied it is routed through the **import**
    /// endpoint (`POST /api/pages/import`), which is the only mechanism that actually
    /// persists page body content — including the Yjs `ydoc` the editor reads from —
    /// across Docmost versions (the JSON `create`/`update` `content` field is silently
    /// dropped on older servers such as v0.25.x). Title-only pages use the plain create
    /// endpoint.
    ///
    /// `parent_page_id` is honored only for title-only pages: the import endpoint always
    /// creates the page at the space root and exposes no parent parameter on v0.25.x.
    pub async fn create_page(
        &self,
        space_id: &str,
        title: &str,
        markdown: Option<&str>,
        parent_page_id: Option<&str>,
    ) -> Result<DocmostPage> {
        if let Some(markdown) = markdown.filter(|markdown| !markdown.trim().is_empty()) {
            // Docmost's importer takes the first level-1 heading as the page title and
            // strips it from the body, so prepend `# {title}` to set the title exactly.
            let document = format!("# {title}\n\n{markdown}");
            return self.import_markdown_page(space_id, &document).await;
        }

        let mut payload = serde_json::json!({
            "spaceId": space_id,
            "title": title,
        });
        if let Some(parent_page_id) = parent_page_id {
            payload["parentPageId"] = Value::String(parent_page_id.to_string());
        }

        self.request::<DocmostPage>("/api/pages/create", payload, true)
            .await
    }

    /// Upload a Markdown document to Docmost's import endpoint, creating a new page with
    /// fully-persisted body content (`content` + `textContent` + `ydoc`). Sends a
    /// multipart form with a `spaceId` text field and a `file` part (the `.md` bytes);
    /// mirrors [`Self::request`]'s bearer auth and single 401-retry.
    pub async fn import_markdown_page(
        &self,
        space_id: &str,
        markdown: &str,
    ) -> Result<DocmostPage> {
        let endpoint = "/api/pages/import";
        let mut session = self.auth_manager.get_authenticated_session().await?;
        let mut retry_on_unauthorized = true;

        loop {
            // A multipart Form cannot be cloned, so it is rebuilt for each attempt.
            let form = reqwest::multipart::Form::new()
                .text("spaceId", space_id.to_string())
                .part(
                    "file",
                    reqwest::multipart::Part::bytes(markdown.as_bytes().to_vec())
                        // Docmost validates the import type by file extension, not MIME,
                        // so the name MUST end in `.md`.
                        .file_name("page.md")
                        .mime_str("text/markdown")?,
                );

            debug_log(
                "api",
                "Importing Docmost page",
                Some(&serde_json::json!({
                    "endpoint": endpoint,
                    "baseUrl": session.base_url,
                    "spaceId": space_id,
                    "retryOnUnauthorized": retry_on_unauthorized
                })),
            );

            let response = self
                .http
                .post(format!("{}{}", session.base_url, endpoint))
                .bearer_auth(&session.token)
                .multipart(form)
                .send()
                .await
                .with_context(|| format!("Failed to call {endpoint}"))?;

            if response.status() == reqwest::StatusCode::UNAUTHORIZED && retry_on_unauthorized {
                session = self.auth_manager.reauthenticate().await?;
                retry_on_unauthorized = false;
                continue;
            }

            return super::parse_response(response).await;
        }
    }

    pub async fn update_page(
        &self,
        page_id: &str,
        title: Option<&str>,
        content: Option<&Value>,
    ) -> Result<DocmostPage> {
        let mut payload = serde_json::json!({ "pageId": page_id });
        if let Some(title) = title {
            payload["title"] = Value::String(title.to_string());
        }
        if let Some(content) = content {
            // Docmost only applies a content change when `content`, `operation`, and
            // `format` are all present; `operation` has no server-side default.
            payload["content"] = content.clone();
            payload["operation"] = Value::String("replace".to_string());
            payload["format"] = Value::String("json".to_string());
        }

        self.request::<DocmostPage>("/api/pages/update", payload, true)
            .await
    }

    /// Duplicate a page. With `space_id`, Docmost's `duplicate` endpoint copies the page
    /// into that space instead — this is how "copy to another space" is expressed. The
    /// server duplicates the stored content (including the Yjs `ydoc`), so the new page's
    /// body persists without going through the import path.
    pub async fn duplicate_page(
        &self,
        page_id: &str,
        space_id: Option<&str>,
    ) -> Result<DocmostPage> {
        let mut payload = serde_json::json!({ "pageId": page_id });
        if let Some(space_id) = space_id {
            payload["spaceId"] = Value::String(space_id.to_string());
        }
        self.request::<DocmostPage>("/api/pages/duplicate", payload, true)
            .await
    }

    /// Move a page (and its subtree) to a different space. Docmost's `move-to-space`
    /// endpoint returns no body, so this resolves to `()` on success.
    pub async fn move_page_to_space(&self, page_id: &str, space_id: &str) -> Result<()> {
        self.request_discard(
            "/api/pages/move-to-space",
            serde_json::json!({ "pageId": page_id, "spaceId": space_id }),
        )
        .await
    }

    /// Move a page under `parent_page_id` (or to the space root when `None`), appended at
    /// the end of that parent's children. This mirrors how the Docmost server positions a
    /// new page — `generateJitteredKeyBetween(lastSiblingPosition, null)` — so the moved
    /// page lands after the current last sibling. Returns the page's state after the move.
    pub async fn move_page(
        &self,
        page_id: &str,
        parent_page_id: Option<&str>,
    ) -> Result<DocmostPage> {
        let page = self
            .get_page(page_id)
            .await?
            .ok_or_else(|| anyhow!("Page to move not found: {page_id}"))?;
        let space_id = page
            .space_id
            .as_deref()
            .ok_or_else(|| anyhow!("Page {page_id} is not in a space"))?;

        let last = self
            .last_sibling_position(space_id, parent_page_id, page.id.as_deref())
            .await?;
        let position = generate_jittered_key_between(last.as_deref(), None)?;

        let mut payload = serde_json::json!({ "pageId": page_id, "position": position });
        if let Some(parent_page_id) = parent_page_id {
            payload["parentPageId"] = Value::String(parent_page_id.to_string());
        }
        self.request_discard("/api/pages/move", payload).await?;

        self.get_page(page_id)
            .await?
            .ok_or_else(|| anyhow!("Page {page_id} not found after move"))
    }

    /// The greatest existing `position` among the target parent's children (or the space's
    /// root pages when `parent_page_id` is `None`), ignoring `exclude_id` (the page being
    /// moved). `None` when there are no other siblings.
    async fn last_sibling_position(
        &self,
        space_id: &str,
        parent_page_id: Option<&str>,
        exclude_id: Option<&str>,
    ) -> Result<Option<String>> {
        let mut payload = serde_json::json!({ "spaceId": space_id });
        if let Some(parent_page_id) = parent_page_id {
            payload["pageId"] = Value::String(parent_page_id.to_string());
        }
        let result: CursorListResult<DocmostPageListItem> = self
            .request("/api/pages/sidebar-pages", payload, true)
            .await?;
        Ok(normalize_cursor_list_result(result)
            .into_iter()
            .filter(|p| Some(p.id.as_str()) != exclude_id)
            .filter_map(|p| p.position)
            .max())
    }

    /// Create a new space (`name` + URL `slug`, optional `description`). Returns the
    /// created space. Requires workspace "manage spaces" permission.
    pub async fn create_space(
        &self,
        name: &str,
        slug: &str,
        description: Option<&str>,
    ) -> Result<DocmostSpace> {
        let mut payload = serde_json::json!({ "name": name, "slug": slug });
        if let Some(description) = description {
            payload["description"] = Value::String(description.to_string());
        }
        self.request::<DocmostSpace>("/api/spaces/create", payload, true)
            .await
    }

    /// Update a space's `name`, `slug`, and/or `description` (each optional; omitted
    /// fields are left unchanged). Requires space "manage settings" permission.
    pub async fn update_space(
        &self,
        space_id: &str,
        name: Option<&str>,
        slug: Option<&str>,
        description: Option<&str>,
    ) -> Result<DocmostSpace> {
        let mut payload = serde_json::json!({ "spaceId": space_id });
        if let Some(name) = name {
            payload["name"] = Value::String(name.to_string());
        }
        if let Some(slug) = slug {
            payload["slug"] = Value::String(slug.to_string());
        }
        if let Some(description) = description {
            payload["description"] = Value::String(description.to_string());
        }
        self.request::<DocmostSpace>("/api/spaces/update", payload, true)
            .await
    }

    /// Add a page-level comment. Docmost stores the comment body as ProseMirror JSON but
    /// the `content` field is validated as a JSON *string*, so the document is serialized
    /// before sending (the server `JSON.parse`s it back). `type` defaults to `page`.
    pub async fn create_comment(&self, page_id: &str, content: &Value) -> Result<DocmostComment> {
        let payload = serde_json::json!({
            "pageId": page_id,
            "content": serde_json::to_string(content).context("Failed to serialize comment content")?,
        });
        self.request::<DocmostComment>("/api/comments/create", payload, true)
            .await
    }

    /// Replace an existing comment's body. Same stringified-`content` contract as create.
    pub async fn update_comment(
        &self,
        comment_id: &str,
        content: &Value,
    ) -> Result<DocmostComment> {
        let payload = serde_json::json!({
            "commentId": comment_id,
            "content": serde_json::to_string(content).context("Failed to serialize comment content")?,
        });
        self.request::<DocmostComment>("/api/comments/update", payload, true)
            .await
    }
}
