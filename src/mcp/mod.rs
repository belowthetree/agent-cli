pub mod mcp_tool;
pub mod mcp_server;
pub mod mcp_manager;
pub use mcp_tool::McpTool;
pub use mcp_manager::*;

use crate::config;

pub async fn init() {
    let config = config::Config::local().unwrap();
    if config.mcp.is_none() {
        println!("没有 mcp");
        return;
    }
    let mcp = config.mcp.unwrap();
    let mgr = mcp_manager::McpManager::global();
    for server in mcp.server.iter() {
        let _ = mgr.add_tool_service(server.name.clone(), server.transport.clone()).await;
    }
}