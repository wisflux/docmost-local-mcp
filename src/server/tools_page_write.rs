//! Page-content write tools: `create_page` and `update_page` (Markdown → ProseMirror).
//!
//! In their own `#[tool_router]` impl (a named `page_write_tool_router`, merged into the
//! server's router in `new()`) to keep each tools file within the size limit.

use rmcp::{handler::server::wrapper::Parameters, model::ErrorData, tool, tool_router};

use crate::{
    prosemirror::markdown_to_prosemirror,
    server::{
        DocmostMcpServer, internal_error,
        render::{format_created_page, format_updated_page},
    },
    types::{CreatePageInput, UpdatePageInput},
};

#[tool_router(router = page_write_tool_router, vis = "pub(crate)")]
impl DocmostMcpServer {
    #[tool(
        name = "create_page",
        description = "Create a new Docmost page in a space from Markdown content.",
        annotations(
            title = "Create Docmost Page",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn create_page(
        &self,
        Parameters(input): Parameters<CreatePageInput>,
    ) -> Result<String, ErrorData> {
        // Markdown body is sent verbatim: the client routes it through Docmost's import
        // endpoint, which converts Markdown -> ProseMirror server-side and persists the
        // body (incl. the Yjs ydoc the editor reads from) on every Docmost version.
        let page = self
            .client
            .create_page(
                &input.space_id,
                &input.title,
                input.markdown.as_deref(),
                input.parent_page_id.as_deref(),
            )
            .await
            .map_err(internal_error)?;

        let mut output = format_created_page(&page, &input.title);
        // Be honest: a page created WITH a body goes through the import endpoint, which has
        // no parent parameter, so `parent_page_id` is silently ignored and the page lands
        // at the space root. Say so rather than report a plain success.
        let has_body = input
            .markdown
            .as_deref()
            .is_some_and(|m| !m.trim().is_empty());
        if has_body && input.parent_page_id.is_some() {
            output.push_str(
                "\n\nNote: this page was created at the space root — parent_page_id is not \
                 applied when a Markdown body is provided (Docmost's import path has no parent \
                 parameter). Use move_page afterwards to nest it under a parent.",
            );
        }
        Ok(output)
    }

    #[tool(
        name = "update_page",
        description = "Update an existing Docmost page's title and/or Markdown content.",
        annotations(
            title = "Update Docmost Page",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn update_page(
        &self,
        Parameters(input): Parameters<UpdatePageInput>,
    ) -> Result<String, ErrorData> {
        let content = input
            .markdown
            .as_deref()
            .filter(|markdown| !markdown.trim().is_empty())
            .map(markdown_to_prosemirror);
        let has_body = content.is_some();
        let page = self
            .client
            .update_page(&input.page_id, input.title.as_deref(), content.as_ref())
            .await
            .map_err(internal_error)?;

        // When a body was sent, tell the caller honestly whether this server actually
        // applies REST body updates (added in Docmost v0.70.0). On older servers the body
        // lives in the collaborative editor and the REST content is silently ignored.
        let body_note = if has_body {
            self.body_update_note().await
        } else {
            None
        };
        Ok(format_updated_page(&page, body_note.as_deref()))
    }

    /// A caveat string for `update_page` when a body was sent but this server may not apply
    /// it over REST. `None` when the server supports REST body updates (no caveat needed).
    async fn body_update_note(&self) -> Option<String> {
        if self.client.capabilities().await.rest_page_body_update {
            return None;
        }
        Some(match self.client.server_version().await {
            Some(version) => format!(
                "Note: this Docmost server (v{version}) does not apply page-body edits over \
                 REST — the body was NOT changed (page bodies are edited through the \
                 collaborative editor before v0.70.0). Create a new page with create_page, \
                 or edit the body in the Docmost app."
            ),
            None => "Note: the Docmost server version could not be determined; if the page \
                     body did not change, this server applies body edits through the \
                     collaborative editor (not REST). Create a new page with create_page instead."
                .to_string(),
        })
    }
}
