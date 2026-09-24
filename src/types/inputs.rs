//! Tool input schemas (`schemars::JsonSchema`) passed to `#[tool]` handlers as
//! `Parameters<T>`. Split from the domain models to keep each file within the size limit.

use schemars::JsonSchema;
use serde::Deserialize;

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
pub struct CreatePageInput {
    #[schemars(description = "The Docmost space ID (UUID) to create the page in.")]
    pub space_id: String,
    #[schemars(description = "Page title.")]
    pub title: String,
    #[serde(default)]
    #[schemars(
        description = "Page body as Markdown; converted to ProseMirror JSON before sending."
    )]
    pub markdown: Option<String>,
    #[serde(default)]
    #[schemars(
        description = "Optional parent page ID to nest this page under (must be in the same space)."
    )]
    pub parent_page_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct UpdatePageInput {
    #[schemars(description = "The Docmost page ID or slug ID to update.")]
    pub page_id: String,
    #[serde(default)]
    #[schemars(description = "Optional new page title. Omit to leave the title unchanged.")]
    pub title: Option<String>,
    #[serde(default)]
    #[schemars(
        description = "Optional new page body as Markdown; replaces the existing content. Omit to leave content unchanged."
    )]
    pub markdown: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct DuplicatePageInput {
    #[schemars(description = "The Docmost page ID or slug ID to duplicate within its space.")]
    pub page_id: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct CopyPageToSpaceInput {
    #[schemars(description = "The Docmost page ID or slug ID to copy.")]
    pub page_id: String,
    #[schemars(description = "The target space ID (UUID) to copy the page into.")]
    pub space_id: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct MovePageInput {
    #[schemars(description = "The Docmost page ID or slug ID to move.")]
    pub page_id: String,
    #[serde(default)]
    #[schemars(
        description = "Optional new parent page ID within the same space. Omit to move the page to the space root. The page is appended after the parent's existing children."
    )]
    pub parent_page_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct MovePageToSpaceInput {
    #[schemars(description = "The Docmost page ID or slug ID to move.")]
    pub page_id: String,
    #[schemars(
        description = "The target space ID (UUID) to move the page (and its sub-pages) into."
    )]
    pub space_id: String,
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

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct CreateSpaceInput {
    #[schemars(description = "Space name (2-100 characters).")]
    pub name: String,
    #[schemars(
        description = "URL slug: letters and numbers only (2-100 characters). Some Docmost versions also allow hyphens and underscores, but letters/numbers work everywhere."
    )]
    pub slug: String,
    #[serde(default)]
    #[schemars(description = "Optional space description.")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct UpdateSpaceInput {
    #[schemars(description = "The space ID (UUID) to update.")]
    pub space_id: String,
    #[serde(default)]
    #[schemars(description = "Optional new name (2-100 characters). Omit to leave unchanged.")]
    pub name: Option<String>,
    #[serde(default)]
    #[schemars(description = "Optional new URL slug. Omit to leave unchanged.")]
    pub slug: Option<String>,
    #[serde(default)]
    #[schemars(description = "Optional new description. Omit to leave unchanged.")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct CreateCommentInput {
    #[schemars(description = "The Docmost page ID (UUID) to comment on.")]
    pub page_id: String,
    #[schemars(
        description = "Comment body as Markdown. Creates a page-level comment (not anchored to a text selection). To tag a user, write a link with a `user:` URL: `[Display Name](user:USER_UUID)` (get USER_UUID from list_workspace_members); `[Title](page:PAGE_UUID)` mentions a page. Tables, images, and task lists are not supported inside comments."
    )]
    pub markdown: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct UpdateCommentInput {
    #[schemars(description = "The comment ID (UUID) to update.")]
    pub comment_id: String,
    #[schemars(
        description = "New comment body as Markdown; replaces the existing content. Tag users with `[Display Name](user:USER_UUID)` (see list_workspace_members)."
    )]
    pub markdown: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct EditPageInput {
    #[schemars(description = "The Docmost page ID or slug ID to edit.")]
    pub page_id: String,
    #[schemars(
        description = "Edits applied in order to the page's ProseMirror document. Every match is \
        against the Markdown that get_page returns and must be exact and unique: an operation \
        that matches zero or several times fails, and then nothing is written."
    )]
    pub operations: Vec<EditOperation>,
    #[serde(default)]
    #[schemars(
        description = "Default true: return the diff and the page's updatedAt, write nothing. \
        Set false (with expected_updated_at) to write."
    )]
    pub dry_run: Option<bool>,
    #[serde(default)]
    #[schemars(
        description = "Required when dry_run is false: the updatedAt value a dry run returned. \
        The write is refused if the page changed since."
    )]
    pub expected_updated_at: Option<String>,
    #[serde(default)]
    #[schemars(
        description = "Default false: refuse an edit that removes the last anchor of an inline \
        comment thread. Set true only when detaching that thread is intended."
    )]
    pub allow_detaching_comments: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum EditOperation {
    /// Replace an exact span of text inside one block (paragraph, heading, code block, table
    /// cell), or a whole table row given as `| a | b |`.
    ReplaceText {
        #[schemars(
            description = "Exact Markdown to find, as get_page shows it (marks included, e.g. \
            `**bold**`). Must lie inside one block, or be one whole table row."
        )]
        find: String,
        #[schemars(
            description = "Inline Markdown that replaces it (one or more table rows, or empty to delete the row, when `find` \
            is a row). Marks that cover the whole match but are not part of `find` are kept."
        )]
        replace: String,
    },
    /// Insert new blocks before or after the top-level block that contains `anchor`.
    InsertBlocks {
        #[schemars(description = "Exact text found in exactly one top-level block.")]
        anchor: String,
        position: InsertPosition,
        #[schemars(description = "Markdown blocks to insert.")]
        markdown: String,
    },
    /// Append one or more rows after the table row that contains `anchor`.
    AppendTableRow {
        #[schemars(
            description = "Exact text found in exactly one table row, as rendered `| a | b |`."
        )]
        anchor: String,
        #[schemars(
            description = "One Markdown table row per line, same cell count as the anchor row."
        )]
        row: String,
    },
    /// Merge adjacent table rows into the first one, on the JSON: the first row's cells are
    /// kept, and the last cell of every other row is appended to its last cell with one space.
    MergeTableRows {
        #[schemars(
            description = "Two or more exact texts, each found in exactly one table row, for \
            adjacent rows of one table, in order."
        )]
        rows: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InsertPosition {
    Before,
    After,
}
