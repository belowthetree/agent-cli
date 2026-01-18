use crate::{
    mcp::mcp_manager,
    model::param::{ModelMessage, ToolCall},
};
use async_stream::stream;
use futures::Stream;
use log::warn;
use serde_json::Value;

pub struct ToolClient;

impl ToolClient {
    pub fn call(
        &self,
        calls: Vec<ToolCall>,
    ) -> impl Stream<Item = Result<ModelMessage, anyhow::Error>> + '_ {
        stream! {
            // 输入验证：检查工具调用列表是否为空
            if calls.is_empty() {
                return;
            }

            // 验证每个工具调用
            for call in calls.iter() {
                // 验证工具名称
                if call.function.name.is_empty() {
                    warn!("工具名称不能为空");
                    // 创建包含错误信息的工具响应，返还给模型
                    let error_content = serde_json::json!({
                        "error": true,
                        "message": "工具名称不能为空",
                        "details": "工具调用缺少名称"
                    }).to_string();
                    yield Ok(ModelMessage::tool(error_content, call.clone()));
                    continue;
                }

                // 解析JSON参数，如果解析失败则返回错误工具响应
                let arguments: Value = match serde_json::from_str(&call.function.arguments) {
                    Ok(args) => args,
                    Err(e) => {
                        warn!("JSON参数解析失败: {}", e);
                        // 创建包含错误信息的工具响应，返还给模型
                        let error_content = serde_json::json!({
                            "error": true,
                            "message": format!("JSON参数解析失败: {}", e),
                            "details": e.to_string()
                        }).to_string();
                        yield Ok(ModelMessage::tool(error_content, call.clone()));
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
                        // 工具调用错误也应该作为工具响应返还给模型
                        let error_content = serde_json::json!({
                            "error": true,
                            "message": format!("工具调用失败: {}", e),
                            "details": e.to_string()
                        }).to_string();
                        warn!("工具调用失败: {} {:?}", error_content, call);
                        yield Ok(ModelMessage::tool(error_content, call.clone()));
                    }
                }
            }
        }
    }
}
