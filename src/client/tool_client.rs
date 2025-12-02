use async_stream::stream;
use futures::Stream;
use serde_json::Value;
use crate::{mcp::mcp_manager, model::param::{ModelMessage, ToolCall}};
use log::warn;

pub struct ToolClient;

impl ToolClient {
    pub fn call(&self, calls: Vec<ToolCall>)-> impl Stream<Item = Result<ModelMessage, anyhow::Error>> + '_ {
        stream! {
            // 输入验证：检查工具调用列表是否为空
            if calls.is_empty() {
                warn!("需要至少调用一个工具");
                yield Err(anyhow::anyhow!("需要至少调用一个工具"));
                return;
            }

            // 验证每个工具调用
            for call in calls.iter() {
                // 验证工具名称
                if call.function.name.is_empty() {
                    warn!("工具名称不能为空");
                    yield Err(anyhow::anyhow!("工具名称不能为空"));
                    continue;
                }

                // 解析JSON参数，如果解析失败则返回错误
                let arguments: Value = match serde_json::from_str(&call.function.arguments) {
                    Ok(args) => args,
                    Err(e) => {
                        warn!("JSON参数解析失败: {}", e);
                        yield Err(anyhow::anyhow!("JSON参数解析失败: {}", e));
                        continue;
                    }
                };

                // 调用工具
                let result = mcp_manager::McpManager::global().call_tool(&call.function.name, &arguments).await;
                
                match result {
                    Ok(s) => {
                        yield Ok(ModelMessage::tool(s, call.clone()));
                    }
                    Err(e) => {
                        // 工具调用错误应该作为错误返回，而不是包装为成功的消息
                        yield Err(anyhow::anyhow!("工具调用失败: {}", e));
                    }
                }
            }
        }
    }
}
