//! Page-structure write tools (duplicate, copy-to-space, move, move-to-space).
//!
//! These live in their own `#[tool_router]` impl (a named `write_tool_router`, merged
//! into the server's router in `new()`) to keep each tools file within the size limit.

use rmcp::{handler::server::wrapper::Parameters, model::ErrorData, tool, tool_router};

use crate::{
    server::{DocmostMcpServer, internal_error, render::format_optional_id},
    types::{
        CopyPageToSpaceInput, DocmostPage, DuplicatePageInput, MovePageInput, MovePageToSpaceInput,
    },
};

#[tool_router(router = write_tool_router, vis = "pub(crate)")]
impl DocmostMcpServer {
    #[tool(
        name = "duplicate_page",
        description = "Duplicate a Docmost page (with its sub-pages) within its space.",
        annotations(
            title = "Duplicate Docmost Page",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn duplicate_page(
        &self,
        Parameters(input): Parameters<DuplicatePageInput>,
    ) -> Result<String, ErrorData> {
        let page = self
            .client
            .duplicate_page(&input.page_id, None)
            .await
            .map_err(internal_error)?;
        Ok(format_duplicated_page(&page, None))
    }

    #[tool(
        name = "copy_page_to_space",
        description = "Copy a Docmost page (with its sub-pages) into a different space.",
        annotations(
            title = "Copy Docmost Page to Space",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn copy_page_to_space(
        &self,
        Parameters(input): Parameters<CopyPageToSpaceInput>,
    ) -> Result<String, ErrorData> {
        let page = self
            .client
            .duplicate_page(&input.page_id, Some(&input.space_id))
            .await
            .map_err(internal_error)?;
        Ok(format_duplicated_page(&page, Some(&input.space_id)))
    }

    #[tool(
        name = "move_page",
        description = "Move a Docmost page under a new parent page, or to the space root. \
                       The page is appended after the target parent's existing children.",
        annotations(
            title = "Move Docmost Page",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = true
        )
    )]
    async fn move_page(
        &self,
        Parameters(input): Parameters<MovePageInput>,
    ) -> Result<String, ErrorData> {
        let page = self
            .client
            .move_page(&input.page_id, input.parent_page_id.as_deref())
            .await
            .map_err(internal_error)?;
        Ok(format_moved_page(&page, input.parent_page_id.as_deref()))
    }

    #[tool(
        name = "move_page_to_space",
        description = "Move a Docmost page (with its sub-pages) to a different space.",
        annotations(
            title = "Move Docmost Page to Space",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = true
        )
    )]
    async fn move_page_to_space(
        &self,
        Parameters(input): Parameters<MovePageToSpaceInput>,
    ) -> Result<String, ErrorData> {
        self.client
            .move_page_to_space(&input.page_id, &input.space_id)
            .await
            .map_err(internal_error)?;
        // move-to-space returns no body; re-fetch to confirm the new location.
        let page = self
            .client
            .get_page(&input.page_id)
            .await
            .map_err(internal_error)?;
        Ok(format_moved_to_space(
            page.as_ref(),
            &input.space_id,
            &input.page_id,
        ))
    }
}

fn format_duplicated_page(page: &DocmostPage, into_space: Option<&str>) -> String {
    let title = page.title.as_deref().unwrap_or("Untitled");
    let header = match into_space {
        Some(space) => format!(
            "Copied Docmost page \"{title}\" into space {}.",
            format_optional_id(Some(space))
        ),
        None => format!("Duplicated Docmost page \"{title}\"."),
    };
    [
        header,
        String::new(),
        format!("New page ID: {}", format_optional_id(page.id.as_deref())),
        format!(
            "New slug ID: {}",
            format_optional_id(page.slug_id.as_deref())
        ),
        format!("Space ID: {}", format_optional_id(page.space_id.as_deref())),
    ]
    .join("\n")
}

fn format_moved_page(page: &DocmostPage, parent_page_id: Option<&str>) -> String {
    let title = page.title.as_deref().unwrap_or("Untitled");
    let location = match parent_page_id {
        Some(parent) => format!("under parent {}", format_optional_id(Some(parent))),
        None => "to the space root".to_string(),
    };
    [
        format!("Moved Docmost page \"{title}\" {location}."),
        String::new(),
        format!("Page ID: {}", format_optional_id(page.id.as_deref())),
        format!("Slug ID: {}", format_optional_id(page.slug_id.as_deref())),
        format!(
            "Parent page ID: {}",
            format_optional_id(page.parent_page_id.as_deref())
        ),
    ]
    .join("\n")
}

fn format_moved_to_space(page: Option<&DocmostPage>, space_id: &str, page_id: &str) -> String {
    let title = page.and_then(|p| p.title.as_deref()).unwrap_or("(page)");
    let id = page.and_then(|p| p.id.as_deref()).unwrap_or(page_id);
    [
        format!(
            "Moved Docmost page \"{title}\" to space {}.",
            format_optional_id(Some(space_id))
        ),
        String::new(),
        format!("Page ID: {}", format_optional_id(Some(id))),
        format!(
            "Slug ID: {}",
            format_optional_id(page.and_then(|p| p.slug_id.as_deref()))
        ),
    ]
    .join("\n")
}
