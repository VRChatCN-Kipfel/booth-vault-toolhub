//! booth-mcp：MCP server 入口。
//!
//! 暴露 download / organize / search / audit 四工具（复用 engine）。
//! stdio 传输，供 opencode 等 agent 调用。

use rmcp::ServiceExt;
use rmcp::transport::stdio;

mod tools;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let service = tools::BoothServer::new();
    let server = service.serve(stdio()).await?;
    server.waiting().await?;
    Ok(())
}
