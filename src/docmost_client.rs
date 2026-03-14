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
