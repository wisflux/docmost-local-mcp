use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StoredConfig {
    pub base_url: String,
    pub email: String,
    pub last_authenticated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StoredSession {
    pub token: String,
    pub expires_at: Option<String>,
    pub saved_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredCredentials {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedSession {
    pub base_url: String,
    pub email: String,
    pub token: String,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StartupConfig {
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LoginInput {
    pub base_url: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthWindowSession {
    pub login_url: String,
    pub success_url: String,
    pub fallback_url: String,
    pub window_title: String,
    pub window_width: u32,
    pub window_height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocmostSpace {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub member_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocmostSearchResult {
    pub id: Option<String>,
    pub slug_id: String,
    pub title: String,
    pub highlight: Option<String>,
    pub icon: Option<String>,
    pub space: Option<DocmostSearchSpace>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocmostSearchSpace {
    pub id: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DocmostPage {
    pub title: String,
    pub icon: Option<String>,
    pub updated_at: Option<String>,
    pub space: Option<DocmostPageSpace>,
    pub creator: Option<DocmostPageCreator>,
    pub content: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocmostPageSpace {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocmostPageCreator {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct SearchDocsInput {
    #[schemars(description = "Full-text query to search for.")]
    pub query: String,
    #[serde(default)]
    #[schemars(description = "Optional Docmost space ID to scope the search.")]
    pub space_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct GetPageInput {
    #[schemars(description = "The page slug ID returned from search_docs.")]
    pub slug_id: String,
}
