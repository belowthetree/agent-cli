use rig::tool::{ToolSet, Tool};
use rig::completion::ToolDefinition;
use serde::{Deserialize, Serialize};
use futures::{future, TryFutureExt};

use crate::config::McpServerTransportConfig;
use crate::mcp::McpManager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    server_name: String,
    use_fake_name: bool,
}

impl McpTool {
    pub fn new(name: String, description: String, input_schema: serde_json::Value, server_name: String, use_fake_name: bool) -> Self {
        Self {
            name,
            description,
            input_schema,
            server_name,
            use_fake_name,
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("McpError error")]
pub struct McpError {
    text: String,
}

impl Tool for McpTool {
    const NAME: &'static str = "McpTool";

    type Error = McpError;

    type Args = String;

    type Output = String;

    fn definition(&self, _prompt: String) -> impl Future<Output = ToolDefinition> + Send + Sync {
        future::ready(ToolDefinition{
            name: self.name(),
            description: self.description.clone(),
            parameters: self.input_schema.clone(),
        })
    }

    fn call(
        &self,
        args: Self::Args,
    ) -> impl Future<Output = Result<Self::Output, Self::Error>> + Send + Sync {
        let server_name = self.server_name.clone();
        let tool_name = self.name.clone();
        let result = McpManager::global().call_tool(&server_name, &tool_name, &args);
        future::ready(result.map_err(|e| McpError{text: e.to_string()}))
    }

    // 获取名字，重名的需要伪造一个名字
    fn name(&self) -> String {
        let mut name = self.name.clone();
        if self.use_fake_name {
            name = self.server_name.clone() + "_" + name.as_str();
        }
        name
    }
}
