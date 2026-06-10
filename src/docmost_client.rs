use anyhow::{Context, Result, anyhow};
use reqwest::{Client, Response};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::{
    auth::manager::{AuthManager, safe_read_response_text},
    debug::debug_log,
    types::{
        DocmostComment, DocmostCurrentUserResponse, DocmostPage, DocmostPageListItem,
        DocmostSearchResult, DocmostSpace, DocmostSpaceWithMembership, DocmostUser,
    },
};

#[derive(Debug, Clone)]
pub struct DocmostClient {
    auth_manager: AuthManager,
    http: Client,
}

#[derive(Debug, serde::Deserialize)]
struct ApiEnvelope<T> {
    data: Option<T>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
pub enum ListResult<T> {
    List(Vec<T>),
    Wrapped { items: Option<Vec<T>> },
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorListResult<T> {
    pub items: Option<Vec<T>>,
}

impl DocmostClient {
    pub fn new(auth_manager: AuthManager) -> Self {
        Self {
            auth_manager,
            http: Client::new(),
        }
    }

    pub async fn list_spaces(&self) -> Result<Vec<DocmostSpace>> {
        let result = self
            .request::<ListResult<DocmostSpace>>(
                "/api/spaces",
                serde_json::json!({ "page": 1, "limit": 100 }),
                true,
            )
            .await?;
        Ok(normalize_list_result(Some(result)))
    }

    pub async fn search_docs(
        &self,
        query: &str,
        space_id: Option<&str>,
    ) -> Result<Vec<DocmostSearchResult>> {
        let mut payload = serde_json::json!({ "query": query });
        if let Some(space_id) = space_id {
            payload["spaceId"] = Value::String(space_id.to_string());
        }

        let result = self
            .request::<ListResult<DocmostSearchResult>>("/api/search", payload, true)
            .await?;
        Ok(normalize_list_result(Some(result)))
    }

    pub async fn get_space(&self, space_id: &str) -> Result<DocmostSpaceWithMembership> {
        self.request(
            "/api/spaces/info",
            serde_json::json!({ "spaceId": space_id }),
            true,
        )
        .await
    }

    pub async fn get_page(&self, slug_id: &str) -> Result<Option<DocmostPage>> {
        self.request(
            "/api/pages/info",
            serde_json::json!({ "pageId": slug_id }),
            true,
        )
        .await
    }

    pub async fn list_pages(
        &self,
        space_id: &str,
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> Result<Vec<DocmostPageListItem>> {
        let mut payload = serde_json::json!({ "spaceId": space_id });
        if let Some(limit) = limit {
            payload["limit"] = Value::from(limit);
        }
        if let Some(cursor) = cursor {
            payload["cursor"] = Value::String(cursor.to_string());
        }

        let result = self
            .request::<CursorListResult<DocmostPageListItem>>("/api/pages/recent", payload, true)
            .await?;
        Ok(normalize_cursor_list_result(result))
    }

    pub async fn list_child_pages(
        &self,
        page_id: &str,
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> Result<Vec<DocmostPageListItem>> {
        let mut payload = serde_json::json!({ "pageId": page_id });
        if let Some(limit) = limit {
            payload["limit"] = Value::from(limit);
        }
        if let Some(cursor) = cursor {
            payload["cursor"] = Value::String(cursor.to_string());
        }

        let result = self
            .request::<CursorListResult<DocmostPageListItem>>(
                "/api/pages/sidebar-pages",
                payload,
                true,
            )
            .await?;
        Ok(normalize_cursor_list_result(result))
    }

    pub async fn get_comments(
        &self,
        page_id: &str,
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> Result<Vec<DocmostComment>> {
        let mut payload = serde_json::json!({ "pageId": page_id });
        if let Some(limit) = limit {
            payload["limit"] = Value::from(limit);
        }
        if let Some(cursor) = cursor {
            payload["cursor"] = Value::String(cursor.to_string());
        }

        let result = self
            .request::<CursorListResult<DocmostComment>>("/api/comments", payload, true)
            .await?;
        Ok(normalize_cursor_list_result(result))
    }

    pub async fn list_workspace_members(
        &self,
        limit: Option<u32>,
        cursor: Option<&str>,
        query: Option<&str>,
        admin_view: Option<bool>,
    ) -> Result<Vec<DocmostUser>> {
        let mut payload = serde_json::json!({});
        if let Some(limit) = limit {
            payload["limit"] = Value::from(limit);
        }
        if let Some(cursor) = cursor {
            payload["cursor"] = Value::String(cursor.to_string());
        }
        if let Some(query) = query {
            payload["query"] = Value::String(query.to_string());
        }
        if let Some(admin_view) = admin_view {
            payload["adminView"] = Value::Bool(admin_view);
        }

        let result = self
            .request::<CursorListResult<DocmostUser>>("/api/workspace/members", payload, true)
            .await?;
        Ok(normalize_cursor_list_result(result))
    }

    pub async fn get_current_user(&self) -> Result<DocmostCurrentUserResponse> {
        self.request("/api/users/me", serde_json::json!({}), true)
            .await
    }

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

            return parse_response(response).await;
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

    async fn request<T>(
        &self,
        endpoint: &str,
        payload: Value,
        retry_on_unauthorized: bool,
    ) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let mut session = self.auth_manager.get_authenticated_session().await?;
        let mut retry_on_unauthorized = retry_on_unauthorized;

        loop {
            debug_log(
                "api",
                "Calling Docmost API",
                Some(&serde_json::json!({
                    "endpoint": endpoint,
                    "baseUrl": session.base_url,
                    "payload": payload,
                    "retryOnUnauthorized": retry_on_unauthorized
                })),
            );

            let response = self
                .http
                .post(format!("{}{}", session.base_url, endpoint))
                .bearer_auth(&session.token)
                .json(&payload)
                .send()
                .await
                .with_context(|| format!("Failed to call {endpoint}"))?;

            debug_log(
                "api",
                "Docmost API response received",
                Some(&serde_json::json!({
                    "endpoint": endpoint,
                    "status": response.status().as_u16(),
                    "ok": response.status().is_success()
                })),
            );

            if response.status() == reqwest::StatusCode::UNAUTHORIZED && retry_on_unauthorized {
                debug_log(
                    "api",
                    "Received 401 from Docmost API; retrying after reauthentication",
                    Some(&serde_json::json!({ "endpoint": endpoint })),
                );
                session = self.auth_manager.reauthenticate().await?;
                retry_on_unauthorized = false;
                continue;
            }

            return parse_response(response).await;
        }
    }
}

pub fn normalize_list_result<T>(result: Option<ListResult<T>>) -> Vec<T> {
    match result {
        Some(ListResult::List(items)) => items,
        Some(ListResult::Wrapped { items }) => items.unwrap_or_default(),
        None => Vec::new(),
    }
}

pub fn normalize_cursor_list_result<T>(result: CursorListResult<T>) -> Vec<T> {
    result.items.unwrap_or_default()
}

async fn parse_response<T>(response: Response) -> Result<T>
where
    T: DeserializeOwned,
{
    if !response.status().is_success() {
        let status = response.status();
        let details = safe_read_response_text(response).await;
        return Err(anyhow!(
            format!("Docmost API request failed ({status}). {details}")
                .trim()
                .to_string()
        ));
    }

    let json = response
        .json::<ApiEnvelope<T>>()
        .await
        .context("Failed to parse Docmost API response body")?;
    json.data
        .ok_or_else(|| anyhow!("Docmost API response was missing a data payload"))
}
