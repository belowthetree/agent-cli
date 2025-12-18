//! 处理 Regenerate 请求的处理器

use super::base_handler::RequestHandler;
use crate::remote::protocol::{RemoteRequest, RemoteResponse};
use crate::config::Config;
use crate::chat::Chat;
use tokio_tungstenite::WebSocketStream;
use tokio::net::TcpStream;
use log::info;
use futures::StreamExt;

/// 处理 Regenerate 请求的处理器
pub struct RegenerateHandler;

#[async_trait::async_trait]
impl RequestHandler for RegenerateHandler {
    async fn handle(
        &self,
        request: RemoteRequest,
        chat: &mut Chat,
        _config: &Config,
        _ws_stream: &mut WebSocketStream<TcpStream>,
    ) -> RemoteResponse {
        info!("Handling regenerate request: {}", request.request_id);

        if !chat.is_running() {
            // 使用 stream_rechat 重新生成回复
            let mut response_chunks = Vec::new();
            
            {
                let stream = chat.stream_rechat();
                futures::pin_mut!(stream);
                
                while let Some(result) = stream.next().await {
                    match result {
                        Ok(response) => {
                            use crate::chat::StreamedChatResponse;
                            match response {
                                StreamedChatResponse::Text(text) => {
                                    response_chunks.push(text);
                                }
                                StreamedChatResponse::Reasoning(think) => {
                                    if !think.is_empty() {
                                        response_chunks.push(format!("[Reasoning: {}]", think));
                                    }
                                }
                                StreamedChatResponse::ToolCall(tool_call) => {
                                    response_chunks.push(format!("[Tool call: {}]", tool_call.function.name));
                                }
                                StreamedChatResponse::ToolResponse(tool_response) => {
                                    if !tool_response.content.is_empty() {
                                        response_chunks.push(format!("[Tool result: {}]", tool_response.content));
                                    }
                                }
                                StreamedChatResponse::TokenUsage(usage) => {
                                    response_chunks.push(format!("{:?}", usage));
                                }
                                StreamedChatResponse::End => {
                                    // End marker, do nothing here
                                }
                            }
                        }
                        Err(e) => {
                            return RemoteResponse::error(&request.request_id, &format!("Regeneration error: {}", e));
                        }
                    }
                }
            }
            let mut str = String::new();
            for s in response_chunks.iter() {
                str = str + &s;
            }
            RemoteResponse {
                request_id: request.request_id,
                response: crate::remote::protocol::ResponseContent::Stream(str),
                error: None,
                token_usage: None,
            }
        } else {
            RemoteResponse::error(&request.request_id, "Cannot regenerate while model is running")
        }
    }
    
    fn can_handle(&self, request: &RemoteRequest) -> bool {
        matches!(&request.input, crate::remote::protocol::InputType::Regenerate)
    }
}
