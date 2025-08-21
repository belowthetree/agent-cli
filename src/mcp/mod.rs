pub mod mcp_tool;
pub mod mcp_server;
pub mod mcp_manager;
pub mod tool_desc;
pub mod internalserver;

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
        println!("{:?}", server.transport);
        let _ = mgr.add_tool_service(server.name.clone(), server.transport.clone()).await;
    }
}