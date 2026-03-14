use rmcp::{handler::server::wrapper::Parameters, model::ErrorData, tool, tool_router};

use crate::{
    auth::manager::AuthManager,
    prosemirror::prosemirror_to_markdown,
    server::{
        DocmostMcpServer, internal_error,
        render::{
            format_comments, format_current_user, format_page_list, format_search_results,
            format_workspace_members,
        },
    },
    types::{
        DocmostSearchResult, EmptyInput, GetCommentsInput, GetPageInput, GetSpaceInput,
        ListChildPagesInput, ListPagesInput, ListWorkspaceMembersInput, SearchDocsInput,
        StartupConfig,
    },
};

impl DocmostMcpServer {
    pub fn new(startup_config: StartupConfig) -> anyhow::Result<Self> {
        let auth_manager = AuthManager::new(startup_config, None)?;
        let client = crate::docmost_client::DocmostClient::new(auth_manager);
        Ok(Self {
            client,
            tool_router: Self::tool_router(),
        })
    }

    async fn search_pages_results(
        &self,
        input: &SearchDocsInput,
    ) -> Result<Vec<DocmostSearchResult>, ErrorData> {
        self.client
            .search_docs(&input.query, input.space_id.as_deref())
            .await
            .map_err(internal_error)
    }
}

#[tool_router]
impl DocmostMcpServer {
    #[tool(
        name = "list_spaces",
        description = "List all available documentation spaces in Docmost, including names, slugs, and IDs.",
        annotations(title = "List Docmost Spaces", read_only_hint = true)
    )]
    async fn list_spaces(
        &self,
        Parameters(_): Parameters<EmptyInput>,
    ) -> Result<String, ErrorData> {
        let spaces = self.client.list_spaces().await.map_err(internal_error)?;

        if spaces.is_empty() {
            return Ok("No Docmost spaces were found.".to_string());
        }

        let mut lines = vec![
            "## Available Documentation Spaces".to_string(),
            String::new(),
            "| Name | Slug | ID |".to_string(),
            "| --- | --- | --- |".to_string(),
        ];

        for space in spaces {
            lines.push(format!(
                "| {} | {} | {} |",
                space.name.as_deref().unwrap_or("Untitled"),
                space.slug,
                space.id
            ));
        }

        let total = lines.len() - 4;
        lines.push(String::new());
        lines.push(format!("Total spaces: {total}"));
        Ok(lines.join("\n"))
    }

    #[tool(
        name = "search_docs",
        description = "Search Docmost documentation and optionally filter results by a space ID from list_spaces.",
        annotations(title = "Search Docmost", read_only_hint = true)
    )]
    async fn search_docs(
        &self,
        Parameters(input): Parameters<SearchDocsInput>,
    ) -> Result<String, ErrorData> {
        let results = self.search_pages_results(&input).await?;
        Ok(format_search_results(&input.query, &results))
    }

    #[tool(
        name = "search_pages",
        description = "Search Docmost pages and optionally filter results by a space ID from list_spaces.",
        annotations(title = "Search Docmost Pages", read_only_hint = true)
    )]
    async fn search_pages(
        &self,
        Parameters(input): Parameters<SearchDocsInput>,
    ) -> Result<String, ErrorData> {
        let results = self.search_pages_results(&input).await?;
        Ok(format_search_results(&input.query, &results))
    }

    #[tool(
        name = "get_space",
        description = "Fetch Docmost space details by space ID, including membership context for the current user.",
        annotations(title = "Get Docmost Space", read_only_hint = true)
    )]
    async fn get_space(
        &self,
        Parameters(input): Parameters<GetSpaceInput>,
    ) -> Result<String, ErrorData> {
        let space = self
            .client
            .get_space(&input.space_id)
            .await
            .map_err(internal_error)?;

        let name = space.name.as_deref().unwrap_or("Untitled");
        let lines = [
            format!("# {name}"),
            String::new(),
            format!("Space ID: `{}`", space.id),
            format!("Slug: `{}`", space.slug),
            format!(
                "Description: {}",
                space.description.as_deref().unwrap_or("None")
            ),
            format!(
                "Visibility: {}",
                space.visibility.as_deref().unwrap_or("Unknown")
            ),
            format!(
                "Member count: {}",
                space
                    .member_count
                    .map(|count| count.to_string())
                    .unwrap_or_else(|| "Unknown".to_string())
            ),
            format!(
                "Your role: {}",
                space
                    .membership
                    .as_ref()
                    .and_then(|membership| membership.role.as_deref())
                    .unwrap_or("Unknown")
            ),
        ];

        Ok(lines.join("\n"))
    }

    #[tool(
        name = "get_page",
        description = "Fetch a Docmost page by slug ID and return its content as Markdown.",
        annotations(title = "Get Docmost Page", read_only_hint = true)
    )]
    async fn get_page(
        &self,
        Parameters(input): Parameters<GetPageInput>,
    ) -> Result<String, ErrorData> {
        let Some(page) = self
            .client
            .get_page(&input.slug_id)
            .await
            .map_err(internal_error)?
        else {
            return Ok(format!(
                "No Docmost page was found for slug ID \"{}\".",
                input.slug_id
            ));
        };

        let markdown = page
            .content
            .as_ref()
            .map(prosemirror_to_markdown)
            .unwrap_or_default();
        let title = match page.icon.as_deref() {
            Some(icon) if !icon.is_empty() => {
                format!("# {icon} {}", page.title.as_deref().unwrap_or("Untitled"))
            }
            _ => format!("# {}", page.title.as_deref().unwrap_or("Untitled")),
        };
        let updated = page
            .updated_at
            .as_deref()
            .map(|value| value.chars().take(10).collect::<String>())
            .unwrap_or_else(|| "Unknown".to_string());

        let lines = [
            title,
            String::new(),
            format!(
                "Space: {}",
                page.space
                    .as_ref()
                    .and_then(|space| space.name.as_deref())
                    .unwrap_or("Unknown")
            ),
            format!(
                "Page ID: {}",
                page.id
                    .as_deref()
                    .map(|value| format!("`{value}`"))
                    .unwrap_or_else(|| "Unknown".to_string())
            ),
            format!("Slug ID: `{}`", input.slug_id),
            format!(
                "Author: {}",
                page.creator
                    .as_ref()
                    .and_then(|creator| creator.name.as_deref())
                    .unwrap_or("Unknown")
            ),
            format!("Last updated: {updated}"),
            String::new(),
            "---".to_string(),
            String::new(),
            markdown,
        ];

        Ok(lines.join("\n").trim().to_string())
    }

    #[tool(
        name = "list_pages",
        description = "List recent Docmost pages in a space by space ID.",
        annotations(title = "List Docmost Pages", read_only_hint = true)
    )]
    async fn list_pages(
        &self,
        Parameters(input): Parameters<ListPagesInput>,
    ) -> Result<String, ErrorData> {
        let pages = self
            .client
            .list_pages(&input.space_id, input.limit, input.cursor.as_deref())
            .await
            .map_err(internal_error)?;
        Ok(format_page_list(
            "Recent Pages",
            &format!("space `{}`", input.space_id),
            &pages,
        ))
    }

    #[tool(
        name = "list_child_pages",
        description = "List child pages for a Docmost page ID.",
        annotations(title = "List Docmost Child Pages", read_only_hint = true)
    )]
    async fn list_child_pages(
        &self,
        Parameters(input): Parameters<ListChildPagesInput>,
    ) -> Result<String, ErrorData> {
        let pages = self
            .client
            .list_child_pages(&input.page_id, input.limit, input.cursor.as_deref())
            .await
            .map_err(internal_error)?;
        Ok(format_page_list(
            "Child Pages",
            &format!("page `{}`", input.page_id),
            &pages,
        ))
    }

    #[tool(
        name = "get_comments",
        description = "List Docmost comments for a page ID.",
        annotations(title = "Get Docmost Comments", read_only_hint = true)
    )]
    async fn get_comments(
        &self,
        Parameters(input): Parameters<GetCommentsInput>,
    ) -> Result<String, ErrorData> {
        let comments = self
            .client
            .get_comments(&input.page_id, input.limit, input.cursor.as_deref())
            .await
            .map_err(internal_error)?;
        Ok(format_comments(&input.page_id, &comments))
    }

    #[tool(
        name = "list_workspace_members",
        description = "List Docmost workspace members visible to the current user.",
        annotations(title = "List Docmost Workspace Members", read_only_hint = true)
    )]
    async fn list_workspace_members(
        &self,
        Parameters(input): Parameters<ListWorkspaceMembersInput>,
    ) -> Result<String, ErrorData> {
        let members = self
            .client
            .list_workspace_members(
                input.limit,
                input.cursor.as_deref(),
                input.query.as_deref(),
                input.admin_view,
            )
            .await
            .map_err(internal_error)?;
        Ok(format_workspace_members(&members))
    }

    #[tool(
        name = "get_current_user",
        description = "Fetch the current Docmost user and workspace context.",
        annotations(title = "Get Current Docmost User", read_only_hint = true)
    )]
    async fn get_current_user(
        &self,
        Parameters(_): Parameters<EmptyInput>,
    ) -> Result<String, ErrorData> {
        let response = self
            .client
            .get_current_user()
            .await
            .map_err(internal_error)?;

        Ok(format_current_user(&response))
    }
}
