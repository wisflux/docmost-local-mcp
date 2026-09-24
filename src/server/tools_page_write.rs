//! Page-content write tools: `create_page` and `update_page` (Markdown → ProseMirror), and
//! `edit_page` (in-place edits on the stored ProseMirror JSON).
//!
//! In their own `#[tool_router]` impl (a named `page_write_tool_router`, merged into the
//! server's router in `new()`) to keep each tools file within the size limit.

use rmcp::{handler::server::wrapper::Parameters, model::ErrorData, tool, tool_router};

use crate::{
    prosemirror::{apply_edits, markdown_to_prosemirror, stored_matches_sent},
    server::{
        DocmostMcpServer, internal_error,
        render::{format_created_page, format_edit_outcome, format_updated_page},
    },
    types::{CreatePageInput, EditPageInput, UpdatePageInput},
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
        description = "Update an existing Docmost page's title and/or Markdown content. The \
            body is rebuilt from Markdown, which detaches inline comment threads; to change \
            part of an existing page, use edit_page instead.",
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

    #[tool(
        name = "edit_page",
        description = "Edit part of an existing page in place, without resending the whole \
            body: replace an exact text span or table row, insert Markdown blocks next to an \
            anchor block, or append table rows. Matches are against get_page's Markdown and \
            must be unique. Everything outside the edited ranges (inline comments, callouts, \
            diagrams, mentions) is kept as is. Runs as a dry run by default and returns a diff \
            plus the page's updatedAt; pass dry_run=false with that expected_updated_at to write.",
        annotations(
            title = "Edit Docmost Page In Place",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn edit_page(
        &self,
        Parameters(input): Parameters<EditPageInput>,
    ) -> Result<String, ErrorData> {
        let invalid = |message: String| ErrorData::invalid_params(message, None);
        let page = self
            .client
            .get_page(&input.page_id)
            .await
            .map_err(internal_error)?
            .ok_or_else(|| {
                invalid(format!(
                    "No Docmost page was found for \"{}\".",
                    input.page_id
                ))
            })?;
        let content = page
            .content
            .as_ref()
            .ok_or_else(|| invalid("The page has no ProseMirror content to edit.".to_string()))?;
        // Without updatedAt there is no lost-update guard, so refuse rather than compare "".
        let updated_at = page.updated_at.clone().ok_or_else(|| {
            invalid("The page has no updatedAt; edit_page cannot guard the write.".to_string())
        })?;

        let outcome =
            apply_edits(content, &input.operations).map_err(|e| invalid(e.to_string()))?;
        // A dry run still shows the diff; the report lists any thread it would detach.
        if input.dry_run.unwrap_or(true) {
            return Ok(format_edit_outcome(&outcome, &updated_at, true));
        }
        if !outcome.detached_comments.is_empty() && !input.allow_detaching_comments.unwrap_or(false)
        {
            return Err(invalid(format!(
                "Refused: the edits remove the last anchor of inline comment thread(s) {}. \
                 Narrow the edit, or set allow_detaching_comments=true if that is intended.",
                outcome.detached_comments.join(", ")
            )));
        }

        // Lost-update guard: the caller reviewed a dry run of this exact revision.
        let expected = input.expected_updated_at.as_deref().ok_or_else(|| {
            invalid(
                "A write needs expected_updated_at: run a dry run first and pass its updatedAt."
                    .to_string(),
            )
        })?;
        if expected != updated_at {
            return Err(invalid(format!(
                "Refused: the page changed since the dry run (updatedAt is now {updated_at}, \
                 expected {expected}). Run the dry run again."
            )));
        }
        // Older servers ignore a REST body; fail rather than report a write that did not happen.
        if let Some(note) = self.body_update_note().await {
            return Err(invalid(note));
        }
        let page_id = page.id.as_deref().unwrap_or(&input.page_id);
        let written = self
            .client
            .update_page(page_id, None, Some(&outcome.doc))
            .await
            .map_err(internal_error)?;

        let mut output =
            format_edit_outcome(&outcome, written.updated_at.as_deref().unwrap_or(""), false);
        let stored = self
            .client
            .get_page(page_id)
            .await
            .map_err(internal_error)?;
        let matches = stored
            .and_then(|p| p.content)
            .is_some_and(|content| stored_matches_sent(&outcome.doc, &content));
        output.push_str(if matches {
            "\n\nVerified: the stored content equals the document that was sent (apart from \
             default attributes the server adds to new nodes)."
        } else {
            "\n\nWarning: the stored content differs from the document that was sent: \
             someone else changed the page, or an open editor added a block. Re-fetch the page \
             and check it."
        });
        // Measured 2026-09-24 against a live Docmost instance: an editor that has the page open inserts an
        // empty paragraph at the top a few seconds after a REST replace.
        output.push_str(
            "\n\nIf someone had the page open in the editor, an empty paragraph may appear at \
             the top a few seconds later; remove it in the editor.",
        );
        Ok(output)
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
