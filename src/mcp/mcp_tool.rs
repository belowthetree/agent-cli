use rmcp::model::Tool;
use serde::{Deserialize, Serialize};
use futures::{future, TryFutureExt};
use crate::mcp::McpManager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    tool: Tool,
    server_name: String,
    use_fake_name: bool,
}

impl McpTool {
    pub fn new(tool: Tool, server_name: String, use_fake_name: bool) -> Self {
        Self {
            tool,
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

impl McpTool {
    pub fn get_tool(&self)->Tool {
        self.tool.clone()
    }

    // 获取名字，重名的需要伪造一个名字
    pub fn name(&self) -> String {
        let mut name = self.tool.name.to_string();
        if self.use_fake_name {
            name = self.server_name.clone() + "_" + name.as_str();
        }
        name
    }

    pub fn origin_name(&self)->String {
        self.tool.name.to_string()
    }

    // 获取服务器名称
    pub fn server_name(&self) -> &str {
        &self.server_name
    }

    pub fn desc(&self)->String {
        self.tool.description.clone().unwrap_or_default().to_string()
    }
}

impl Into<Tool> for McpTool{
    fn into(self) -> Tool {
        self.tool
    }
}
