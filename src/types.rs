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
#[serde(rename_all = "camelCase")]
pub struct DocmostSpace {
    pub id: String,
    pub name: Option<String>,
    pub slug: String,
    pub description: Option<String>,
    pub member_count: Option<i64>,
    pub visibility: Option<String>,
    pub default_role: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocmostSpaceWithMembership {
    pub id: String,
    pub name: Option<String>,
    pub slug: String,
    pub description: Option<String>,
    pub member_count: Option<i64>,
    pub visibility: Option<String>,
    pub default_role: Option<String>,
    pub membership: Option<DocmostSpaceMembership>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocmostSpaceMembership {
    pub user_id: Option<String>,
    pub role: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocmostSearchResult {
    pub id: Option<String>,
    pub slug_id: String,
    pub title: Option<String>,
    pub highlight: Option<String>,
    pub icon: Option<String>,
    pub parent_page_id: Option<String>,
    pub creator_id: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub space: Option<DocmostSearchSpace>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocmostSearchSpace {
    pub id: Option<String>,
    pub name: Option<String>,
    pub slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DocmostPage {
    pub id: Option<String>,
    pub slug_id: Option<String>,
    pub title: Option<String>,
    pub icon: Option<String>,
    pub updated_at: Option<String>,
    pub parent_page_id: Option<String>,
    pub space_id: Option<String>,
    pub space: Option<DocmostPageSpace>,
    pub creator: Option<DocmostPageCreator>,
    pub content: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocmostPageSpace {
    pub id: Option<String>,
    pub name: Option<String>,
    pub slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocmostPageCreator {
    pub id: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocmostPageListItem {
    pub id: String,
    pub slug_id: String,
    pub title: Option<String>,
    pub icon: Option<String>,
    pub parent_page_id: Option<String>,
    pub has_children: Option<bool>,
    pub space_id: Option<String>,
    pub updated_at: Option<String>,
    pub space: Option<DocmostSearchSpace>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DocmostComment {
    pub id: String,
    pub page_id: Option<String>,
    pub content: Option<Value>,
    pub selection: Option<String>,
    pub parent_comment_id: Option<String>,
    pub creator: Option<DocmostUserSummary>,
    pub resolved_by: Option<DocmostUserSummary>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub resolved_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocmostUserSummary {
    pub id: Option<String>,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocmostUser {
    pub id: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
    pub role: Option<String>,
    pub deactivated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocmostWorkspace {
    pub id: String,
    pub name: Option<String>,
    pub hostname: Option<String>,
    pub default_space_id: Option<String>,
    pub member_count: Option<i64>,
    pub has_license_key: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocmostCurrentUserResponse {
    pub user: DocmostUser,
    pub workspace: DocmostWorkspace,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Default)]
pub struct EmptyInput {}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct SearchDocsInput {
    #[schemars(description = "Full-text query to search for.")]
    pub query: String,
    #[serde(default)]
    #[schemars(description = "Optional Docmost space ID to scope the search.")]
    pub space_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct GetSpaceInput {
    #[schemars(description = "The Docmost space ID.")]
    pub space_id: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct GetPageInput {
    #[schemars(description = "The page slug ID returned from search_docs.")]
    pub slug_id: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ListPagesInput {
    #[schemars(description = "The Docmost space ID to list pages from.")]
    pub space_id: String,
    #[serde(default)]
    #[schemars(description = "Optional number of pages to return.")]
    pub limit: Option<u32>,
    #[serde(default)]
    #[schemars(description = "Optional cursor returned by a previous list call.")]
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ListChildPagesInput {
    #[schemars(description = "The parent Docmost page ID.")]
    pub page_id: String,
    #[serde(default)]
    #[schemars(description = "Optional number of child pages to return.")]
    pub limit: Option<u32>,
    #[serde(default)]
    #[schemars(description = "Optional cursor returned by a previous list call.")]
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct GetCommentsInput {
    #[schemars(description = "The Docmost page ID to list comments for.")]
    pub page_id: String,
    #[serde(default)]
    #[schemars(description = "Optional number of comments to return.")]
    pub limit: Option<u32>,
    #[serde(default)]
    #[schemars(description = "Optional cursor returned by a previous list call.")]
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ListWorkspaceMembersInput {
    #[serde(default)]
    #[schemars(description = "Optional number of members to return.")]
    pub limit: Option<u32>,
    #[serde(default)]
    #[schemars(description = "Optional cursor returned by a previous list call.")]
    pub cursor: Option<String>,
    #[serde(default)]
    #[schemars(description = "Optional text filter for member names or emails.")]
    pub query: Option<String>,
    #[serde(default)]
    #[schemars(description = "Optional admin view flag, when supported by the workspace.")]
    pub admin_view: Option<bool>,
}
