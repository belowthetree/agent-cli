use async_stream::stream;
use futures::Stream;
use serde_json::Value;
use crate::{mcp::mcp_manager, model::param::{ModelMessage, ToolCall}};
use log::warn;

pub struct ToolClient;

impl ToolClient {
    pub fn call(&self, calls: Vec<ToolCall>)-> impl Stream<Item = Result<ModelMessage, anyhow::Error>> + '_ {
        stream! {
            if calls.len() <= 0 {
                warn!("需要至少调用一个工具");
                yield Err(anyhow::anyhow!("需要至少调用一个工具"));
                return;
            }
            for call in calls.iter() {
                let arguments: Value = serde_json::from_str(&call.function.arguments)
                .unwrap_or_else(|_| Value::Object(serde_json::Map::new()));
                let result = mcp_manager::McpManager::global().call_tool(&call.function.name, &arguments).await;
                match result {
                    Ok(s) => {
                        yield Ok(ModelMessage::tool(s, call.clone()));
                    }
                    Err(e) => {
                        yield Ok(ModelMessage::tool(e.to_string(), call.clone()));
                    }
                }
            }
        }
    }
}