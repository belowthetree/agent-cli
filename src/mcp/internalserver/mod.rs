use std::fmt::Debug;

use rmcp::model::{CallToolResult, Tool};
use serde_json::{Map, Value};

pub mod choosetool;
pub mod getbesttool;

pub trait InternalTool: Send + Sync + Debug {
    fn call(&self, args: Map<String, Value>)->anyhow::Result<CallToolResult>;
    fn get_mcp_tool(&self)->Tool;
}
