use rmcp::{
    ServerHandler,
    handler::server::router::tool::ToolRouter,
    model::{ErrorData, ServerCapabilities, ServerInfo},
    tool_handler,
};

use crate::docmost_client::DocmostClient;

mod render;
mod tools;
mod tools_write;

#[derive(Debug, Clone)]
pub struct DocmostMcpServer {
    client: DocmostClient,
    tool_router: ToolRouter<Self>,
}

#[tool_handler]
impl ServerHandler for DocmostMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Docmost MCP server for listing spaces, searching docs, and fetching pages, plus creating and updating pages from Markdown, organizing pages (duplicate, move, and copy or move between spaces), and creating or updating spaces."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

fn internal_error(error: anyhow::Error) -> ErrorData {
    ErrorData::internal_error(error.to_string(), None)
}
