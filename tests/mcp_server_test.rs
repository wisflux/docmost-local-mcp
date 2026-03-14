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

    for expected in [
        "list_spaces",
        "search_docs",
        "search_pages",
        "get_space",
        "get_page",
        "list_pages",
        "list_child_pages",
        "get_comments",
        "list_workspace_members",
        "get_current_user",
    ] {
        assert!(
            tool_names.iter().any(|name| name == expected),
            "missing tool {expected}"
        );
    }

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

#[tokio::test]
async fn server_all_tools_expose_object_input_schemas() -> Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(16 * 1024);

    let server_handle = tokio::spawn(async move {
        let server = DocmostMcpServer::new(StartupConfig::default())?;
        server.serve(server_transport).await?.waiting().await?;
        anyhow::Ok(())
    });

    let client = DummyClientHandler.serve(client_transport).await?;
    let tools = client.list_tools(None).await?;
    for tool in tools.tools {
        assert_eq!(
            tool.input_schema
                .get("type")
                .and_then(|value| value.as_str()),
            Some("object"),
            "tool {} must expose object input schema",
            tool.name
        );
    }

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

#[tokio::test]
async fn server_required_input_fields_are_present() -> Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(16 * 1024);

    let server_handle = tokio::spawn(async move {
        let server = DocmostMcpServer::new(StartupConfig::default())?;
        server.serve(server_transport).await?.waiting().await?;
        anyhow::Ok(())
    });

    let client = DummyClientHandler.serve(client_transport).await?;
    let tools = client.list_tools(None).await?;

    for (tool_name, property_name) in [
        ("get_page", "slug_id"),
        ("get_space", "space_id"),
        ("list_pages", "space_id"),
        ("list_child_pages", "page_id"),
        ("get_comments", "page_id"),
        ("search_docs", "query"),
        ("search_pages", "query"),
    ] {
        let tool = tools
            .tools
            .iter()
            .find(|tool| tool.name == tool_name)
            .unwrap_or_else(|| panic!("{tool_name} tool should exist"));
        let properties = tool
            .input_schema
            .get("properties")
            .and_then(|value| value.as_object())
            .unwrap_or_else(|| panic!("{tool_name} should expose properties"));
        assert!(
            properties.contains_key(property_name),
            "{tool_name} should contain property {property_name}"
        );
    }

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}
