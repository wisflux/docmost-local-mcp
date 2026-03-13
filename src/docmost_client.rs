use anyhow::{Context, Result, anyhow};
use reqwest::{Client, Response};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::{
    auth::manager::{AuthManager, safe_read_response_text},
    debug::debug_log,
    types::{DocmostPage, DocmostSearchResult, DocmostSpace},
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

    pub async fn get_page(&self, slug_id: &str) -> Result<Option<DocmostPage>> {
        self.request(
            "/api/pages/info",
            serde_json::json!({ "pageId": slug_id }),
            true,
        )
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
