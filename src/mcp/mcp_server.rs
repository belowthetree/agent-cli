use std::sync::Arc;

use crate::config::McpServerTransportConfig;
use crate::mcp::internalserver::InternalTool;

#[derive(Debug, Clone)]
pub enum McpService {
    Common(McpServerTransportConfig),
    Internal(Arc<dyn InternalTool>),
}

impl McpService {
    pub fn from_config(config: McpServerTransportConfig) -> Self {
        Self::Common(config)
    }

    pub fn from_internal(tool: Arc<dyn InternalTool>) -> Self {
        Self::Internal(tool)
    }
}
