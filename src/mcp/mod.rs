pub mod mcp_tool;
pub mod mcp_server;
pub mod mcp_manager;
pub mod tool_desc;
pub mod internalserver;

use log::{info, warn};
pub use mcp_tool::McpTool;
pub use mcp_manager::*;

use crate::{config, mcp::internalserver::{getbesttool::GetBestTool, InternalTool}};

pub async fn init() {
    let config = config::Config::local().unwrap();
    if config.mcp.is_none() {
        warn!("没有 mcp");
        return;
    }
    let mcp = config.mcp.unwrap();
    let mgr = mcp_manager::McpManager::global();
    info!("{:?}", mcp);
    for server in mcp.server.iter() {
        info!("{:?}", server.transport);
        let e = mgr.add_tool_service(server.name.clone(), server.transport.clone()).await;
        if e.is_err() {
            log::error!("{:?}", e);
        }
    }
    let tools = get_config_tools();
    for tool in tools.iter() {
        info!("{}", tool.name());
    }
    // let _ = mgr.add_internal_tool(Arc::new(GetBestTool));
}

pub fn get_basic_tools()->Vec<McpTool> {
    vec![
        McpTool::new(GetBestTool.get_mcp_tool(), "".into(), false),
    ]
}

pub fn get_config_tools()->Vec<McpTool> {
    mcp_manager::McpManager::global().get_all_tools()
}