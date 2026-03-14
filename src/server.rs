use once_cell::sync::Lazy;
use regex::Regex;
use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ErrorData, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
};

use crate::{
    auth::manager::AuthManager,
    docmost_client::DocmostClient,
    prosemirror::prosemirror_to_markdown,
    types::{EmptyInput, GetPageInput, SearchDocsInput, StartupConfig},
};

static HIGHLIGHT_TAGS_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"<[^>]+>").expect("valid highlight strip regex"));
static COLLAPSE_WHITESPACE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\s+").expect("valid whitespace collapse regex"));

#[derive(Debug, Clone)]
pub struct DocmostMcpServer {
    client: DocmostClient,
    tool_router: ToolRouter<Self>,
}

impl DocmostMcpServer {
    pub fn new(startup_config: StartupConfig) -> anyhow::Result<Self> {
        let auth_manager = AuthManager::new(startup_config, None)?;
        let client = DocmostClient::new(auth_manager);
        Ok(Self {
            client,
            tool_router: Self::tool_router(),
        })
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
                space.name, space.slug, space.id
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
        let results = self
            .client
            .search_docs(&input.query, input.space_id.as_deref())
            .await
            .map_err(internal_error)?;

        if results.is_empty() {
            return Ok(format!(
                "No Docmost results were found for \"{}\".",
                input.query
            ));
        }

        let mut lines = vec![
            format!("## Search Results for \"{}\"", input.query),
            String::new(),
        ];
        let total_results = results.len();

        for (index, result) in results.iter().take(5).enumerate() {
            let space_name = result
                .space
                .as_ref()
                .and_then(|space| space.name.as_deref())
                .unwrap_or("Unknown");
            let preview = sanitize_highlight(result.highlight.as_deref());
            let icon = result.icon.as_deref().unwrap_or("");

            if icon.is_empty() {
                lines.push(format!("### {}. {}", index + 1, result.title));
            } else {
                lines.push(format!("### {}. {} {}", index + 1, icon, result.title));
            }
            lines.push(format!("- Space: {space_name}"));
            lines.push(format!("- Slug ID: `{}`", result.slug_id));
            if !preview.is_empty() {
                lines.push(format!("- Preview: {preview}"));
            }
            lines.push(String::new());
        }

        lines.push(format!(
            "Showing {} of {} results.",
            results.iter().take(5).count(),
            total_results
        ));
        lines.push("Use `get_page` with a slug ID to retrieve the full page.".to_string());
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
            Some(icon) if !icon.is_empty() => format!("# {icon} {}", page.title),
            _ => format!("# {}", page.title),
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
}

#[tool_handler]
impl ServerHandler for DocmostMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Read-only Docmost MCP server for listing spaces, searching docs, and fetching pages."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

fn sanitize_highlight(value: Option<&str>) -> String {
    let Some(value) = value else {
        return String::new();
    };
    COLLAPSE_WHITESPACE_RE
        .replace_all(&HIGHLIGHT_TAGS_RE.replace_all(value, ""), " ")
        .trim()
        .to_string()
}

fn internal_error(error: anyhow::Error) -> ErrorData {
    ErrorData::internal_error(error.to_string(), None)
}
