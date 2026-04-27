//! Stdio MCP server that proxies tool calls into the running harness
//! desktop app via its local TCP bridge. Bundled with the
//! Claude Code `harness-plugin` so the user's Claude can read/write
//! the harness knowledge graph.

use rmcp::ServiceExt;

mod server;

const BRIDGE_PORT: u16 = 19851;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_writer(std::io::stderr)
        .init();

    tracing::info!("Starting harness-mcp (bridge on port {BRIDGE_PORT})");

    if let Err(e) = tokio::net::TcpStream::connect(format!("127.0.0.1:{BRIDGE_PORT}")).await {
        tracing::error!("Cannot connect to harness app bridge on port {BRIDGE_PORT}: {e}");
        tracing::error!("Make sure the harness desktop app is running.");
        anyhow::bail!("harness app not running — bridge unavailable on port {BRIDGE_PORT}");
    }
    tracing::info!("Bridge connection verified");

    let service = server::HarnessMcpServer::new(BRIDGE_PORT)
        .serve(rmcp::transport::stdio())
        .await
        .map_err(|e| anyhow::anyhow!("Failed to start MCP server: {e}"))?;

    tracing::info!("harness-mcp running on stdio");
    service.waiting().await?;
    tracing::info!("harness-mcp shutting down");
    Ok(())
}
