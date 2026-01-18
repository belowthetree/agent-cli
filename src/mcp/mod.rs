pub mod internalserver;
pub mod mcp_manager;
pub mod mcp_server;
pub mod mcp_tool;

use log::{info, warn};
pub use mcp_manager::*;
pub use mcp_tool::McpTool;
use std::sync::Arc;

use crate::{
    config,
    mcp::internalserver::{
        InternalTool, filesystem::FileSystemTool, shell_command::ShellCommandTool,
    },
};

pub async fn init() {
    let config = config::Config::local().unwrap();
    if config.mcp.is_none() {
        warn!("没有 mcp");
    }
    let mgr = mcp_manager::McpManager::global();
    if let Some(mcp) = config.mcp {
        info!("{:?}", mcp);
        for server in mcp.server.iter() {
            info!("{:?}", server);
            let e = mgr
                .add_tool_service(server.0.clone(), server.1.transport.clone())
                .await;
            if e.is_err() {
                log::error!("{:?}", e);
            }
        }
        let tools = get_config_tools();
        for tool in tools.iter() {
            info!("{}", tool.name());
        }
    }
    let _ = mgr.add_internal_tool(Arc::new(FileSystemTool));
    let _ = mgr.add_internal_tool(Arc::new(ShellCommandTool));
}

#[allow(unused)]
pub fn get_basic_tools() -> Vec<McpTool> {
    vec![McpTool::new(
        FileSystemTool.get_mcp_tool(),
        "".into(),
        false,
    )]
}

pub fn get_config_tools() -> Vec<McpTool> {
    mcp_manager::McpManager::global().get_all_tools()
}
