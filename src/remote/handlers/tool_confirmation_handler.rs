//! 处理 ToolConfirmationResponse 请求的处理器

use super::base_handler::RequestHandler;
use crate::chat::{Chat, EChatState};
use crate::config::Config;
use crate::remote::protocol::{InputType, RemoteRequest, RemoteResponse};
use log::{info, warn};
use tokio::net::TcpStream;
use tokio_tungstenite::WebSocketStream;

/// 处理 ToolConfirmationResponse 请求的处理器
pub struct ToolConfirmationHandler;

#[async_trait::async_trait]
impl RequestHandler for ToolConfirmationHandler {
    async fn handle(
        &self,
        request: RemoteRequest,
        chat: &mut Chat,
        _config: &Config,
        _ws_stream: &mut WebSocketStream<TcpStream>,
    ) -> RemoteResponse {
        let InputType::ToolConfirmationResponse {
            name,
            arguments,
            approved,
            reason,
        } = &request.input
        else {
            return RemoteResponse::error(
                &request.request_id,
                "Invalid request type for ToolConfirmationHandler",
            );
        };

        info!(
            "Handling tool confirmation response: {} - tool: {}, approved: {}",
            request.request_id, name, approved
        );

        if chat.get_state() == EChatState::WaitingToolConfirm {
            // 验证工具名称和参数是否匹配
            let mut validation_error = None;

            // 获取等待确认的工具调用
            if let Some(last_msg) = chat.context().last() {
                if let Some(tool_calls) = &last_msg.tool_calls {
                    if let Some(tool_call) = tool_calls.first() {
                        // 验证工具名称
                        if tool_call.function.name != *name {
                            validation_error = Some(format!(
                                "Tool name mismatch: expected '{}', got '{}'",
                                tool_call.function.name, name
                            ));
                        } else {
                            // 验证参数（可选，因为参数可能被序列化为字符串）
                            // 尝试解析工具调用中的参数
                            match serde_json::from_str::<serde_json::Value>(
                                &tool_call.function.arguments,
                            ) {
                                Ok(expected_args) => {
                                    // 简单比较JSON值是否相等
                                    if &expected_args != arguments {
                                        warn!("Tool arguments mismatch for tool '{}'", name);
                                        // 这里不视为错误，因为参数格式可能不同
                                    }
                                }
                                Err(e) => {
                                    warn!("Failed to parse tool arguments for validation: {}", e);
                                }
                            }
                        }
                    } else {
                        validation_error = Some("No tool call found in last message".to_string());
                    }
                } else {
                    validation_error = Some("No tool calls found in last message".to_string());
                }
            } else {
                validation_error = Some("No last message found in chat context".to_string());
            }

            // 如果有验证错误，返回错误响应
            if let Some(error_msg) = validation_error {
                return RemoteResponse::error(
                    &request.request_id,
                    &format!("Tool confirmation validation failed: {}", error_msg),
                );
            }

            // 设置工具确认结果
            if *approved {
                chat.confirm();
            } else {
                chat.reject_tool_call();
            }

            // 如果有原因，记录下来
            if let Some(reason) = reason {
                info!("Tool confirmation reason: {}", reason);
            }

            // 继续处理工具调用
            let result: Result<RemoteResponse, ()> = Ok(RemoteResponse {
                request_id: String::new(),
                response: crate::remote::protocol::ResponseContent::Text(format!(
                    "Tool '{}' execution was not approved by user",
                    name
                )),
                error: None,
                token_usage: None,
            });

            match result {
                Ok(mut response) => {
                    response.request_id = request.request_id;
                    response
                }
                Err(e) => RemoteResponse::error(
                    &request.request_id,
                    &format!("Tool confirmation processing error: {:?}", e),
                ),
            }
        } else {
            RemoteResponse::error(&request.request_id, "No pending tool confirmation found")
        }
    }

    fn can_handle(&self, request: &RemoteRequest) -> bool {
        matches!(&request.input, InputType::ToolConfirmationResponse { .. })
    }
}
