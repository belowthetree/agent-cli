use std::fmt::Debug;
use async_trait::async_trait;
use rmcp::model::{CallToolResult, Tool};
use serde_json::{Map, Value};

pub mod choosetool;
pub mod getbesttool;
pub mod filesystem;

#[async_trait]
pub trait InternalTool: Send + Sync + Debug {
    async fn call(&self, args: Map<String, Value>)->anyhow::Result<CallToolResult>;
    fn get_mcp_tool(&self)->Tool;
    fn name(&self)->String;
}
