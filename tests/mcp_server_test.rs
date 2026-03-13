use anyhow::Result;
use docmost_local_mcp::{server::DocmostMcpServer, types::StartupConfig};
use rmcp::{
    ClientHandler, ServiceExt,
    model::{CallToolRequestParam, ClientInfo},
};

#[derive(Debug, Clone, Default)]
struct DummyClientHandler;

impl ClientHandler for DummyClientHandler {
    fn get_info(&self) -> ClientInfo {
        ClientInfo::default()
    }
}

#[tokio::test]
async fn server_lists_expected_tools() -> Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(16 * 1024);

    let server_handle = tokio::spawn(async move {
        let server = DocmostMcpServer::new(StartupConfig::default())?;
        server.serve(server_transport).await?.waiting().await?;
        anyhow::Ok(())
    });

    let client = DummyClientHandler.serve(client_transport).await?;
    let tools = client.list_tools(None).await?;
    let tool_names = tools
        .tools
        .iter()
        .map(|tool| tool.name.to_string())
        .collect::<Vec<_>>();

    assert!(tool_names.iter().any(|name| name == "list_spaces"));
    assert!(tool_names.iter().any(|name| name == "search_docs"));
    assert!(tool_names.iter().any(|name| name == "get_page"));

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

#[tokio::test]
async fn server_get_page_requires_slug_id_schema() -> Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(16 * 1024);

    let server_handle = tokio::spawn(async move {
        let server = DocmostMcpServer::new(StartupConfig::default())?;
        server.serve(server_transport).await?.waiting().await?;
        anyhow::Ok(())
    });

    let client = DummyClientHandler.serve(client_transport).await?;
    let tools = client.list_tools(None).await?;
    let get_page = tools
        .tools
        .into_iter()
        .find(|tool| tool.name == "get_page")
        .expect("get_page tool should exist");
    let properties = get_page
        .input_schema
        .get("properties")
        .and_then(|value| value.as_object())
        .expect("get_page tool should expose properties");

    assert!(properties.contains_key("slug_id"));

    let error = client
        .call_tool(CallToolRequestParam {
            name: "get_page".into(),
            arguments: Some(serde_json::Map::new()),
        })
        .await
        .expect_err("missing slug_id should be rejected");
    assert!(error.to_string().contains("slug_id"));

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}
