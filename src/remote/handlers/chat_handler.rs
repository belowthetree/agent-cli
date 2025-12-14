//! 处理普通聊天请求的处理器

use super::base_handler::RequestHandler;
use crate::remote::protocol::{RemoteRequest, RemoteResponse};
use crate::config::Config;
use crate::chat::Chat;
use crate::mcp;
use tokio_tungstenite::WebSocketStream;
use tokio::net::TcpStream;
use log::{info};
use anyhow::Result;

/// 处理普通聊天请求的处理器
pub struct ChatHandler;

#[async_trait::async_trait]
impl RequestHandler for ChatHandler {
    async fn handle(
        &self,
        request: RemoteRequest,
        chat: &mut Chat,
        config: &Config,
        ws_stream: &mut WebSocketStream<TcpStream>,
    ) -> RemoteResponse {
        info!("Handling chat request: {}", request.request_id);
        
        // Extract text from input
        let input_text = request.input.to_text();
        
        // Merge request config with default config
        let mut chat_config = config.clone();
        if let Some(req_config) = &request.config {
            if let Some(max_tool_try) = req_config.max_tool_try {
                chat_config.max_tool_try = max_tool_try;
            }
            if let Some(max_context_num) = req_config.max_context_num {
                chat_config.max_context_num = max_context_num;
            }
            if let Some(max_tokens) = req_config.max_tokens {
                chat_config.max_tokens = Some(max_tokens);
            }
            if let Some(ask_before_tool_execution) = req_config.ask_before_tool_execution {
                chat_config.ask_before_tool_execution = ask_before_tool_execution;
            }
            if let Some(prompt) = &req_config.prompt {
                chat_config.prompt = Some(prompt.clone());
            }
        }
        
        // Configure tools if requested
        let use_tools = request.use_tools.unwrap_or(true);
        if use_tools {
            // Ensure tools are configured
            chat.set_tools(mcp::get_config_tools());
            chat.set_tools(mcp::get_basic_tools());
        }
        
        // Process the chat request with WebSocket
        let result = self.process_chat_with_ws(ws_stream, chat, &input_text).await;
        
        match result {
            Ok(mut response) => {
                // Set the correct request ID
                response.request_id = request.request_id;
                response
            }
            Err(e) => RemoteResponse::error(&request.request_id, &format!("Processing error: {}", e)),
        }
    }
    
    fn can_handle(&self, _request: &RemoteRequest) -> bool {
        // ChatHandler handles all requests that are not handled by other specific handlers
        // This is determined by the HandlerFactory
        true
    }
}

impl ChatHandler {
    /// 处理聊天请求（无WebSocket交互）
    async fn process_chat(
        &self,
        chat: &mut Chat,
        input: &str,
    ) -> Result<RemoteResponse> {
        use futures::StreamExt;
        
        let mut response_chunks = Vec::new();
        let mut tool_errors = Vec::new();
        
        // Consume the stream in an inner scope to ensure it's dropped before accessing chat.context
        {
            let stream = chat.stream_chat(input);
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
                                    // Check if the tool response contains an error
                                    if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&tool_response.content) {
                                        if let Some(error_field) = json_value.get("error") {
                                            if error_field == true {
                                                // This is a tool error, extract details
                                                let tool_name = if let Some(tool_calls) = &tool_response.tool_calls {
                                                    if let Some(tool_call) = tool_calls.first() {
                                                        tool_call.function.name.clone()
                                                    } else {
                                                        "unknown".to_string()
                                                    }
                                                } else {
                                                    "unknown".to_string()
                                                };
                                                
                                                let error_message = json_value.get("message")
                                                    .and_then(|m| m.as_str())
                                                    .unwrap_or("Tool execution failed");
                                                
                                                let arguments = if let Some(tool_calls) = &tool_response.tool_calls {
                                                    if let Some(tool_call) = tool_calls.first() {
                                                        match serde_json::from_str(&tool_call.function.arguments) {
                                                            Ok(args) => Some(args),
                                                            Err(_) => None,
                                                        }
                                                    } else {
                                                        None
                                                    }
                                                } else {
                                                    None
                                                };
                                                
                                                tool_errors.push((tool_name, error_message.to_string(), arguments));
                                            }
                                        }
                                    }
                                    
                                    response_chunks.push(format!("[Tool result: {}]", tool_response.content));
                                }
                            }
                            StreamedChatResponse::End => {
                                // End marker, do nothing here
                            }
                        }
                    }
                    Err(e) => {
                        return Err(anyhow::anyhow!("Chat error: {}", e));
                    }
                }
            }
        } // stream is dropped here, releasing the mutable borrow on chat
        
        // Check if chat is waiting for tool confirmation
        if chat.is_waiting_tool_confirmation() {
            // Get the last tool call from context
            if let Some(last_msg) = chat.context().last() {
                if let Some(tool_calls) = &last_msg.tool_calls {
                    if let Some(tool_call) = tool_calls.first() {
                        // Parse arguments string to JSON value
                        let arguments: serde_json::Value = match serde_json::from_str(&tool_call.function.arguments) {
                            Ok(args) => args,
                            Err(e) => {
                                // If parsing fails, create an empty object
                                log::warn!("Failed to parse tool arguments as JSON: {}", e);
                                serde_json::json!({})
                            }
                        };
                        
                        // Return a tool confirmation request
                        return Ok(RemoteResponse {
                            request_id: String::new(), // Will be replaced by caller
                            response: crate::remote::protocol::ResponseContent::ToolConfirmationRequest {
                                name: tool_call.function.name.clone(),
                                arguments,
                                description: None,
                            },
                            error: None,
                            token_usage: None,
                        });
                    }
                }
            }
        }
        
        // Get token usage from last message if available
        let token_usage = chat.context().last().and_then(|last_msg| {
            last_msg.token_usage.as_ref().map(|usage| crate::remote::protocol::TokenUsage {
                prompt_tokens: usage.prompt_tokens,
                completion_tokens: usage.completion_tokens,
                total_tokens: usage.total_tokens,
            })
        });
        
        // If there are tool errors, return a tool error response
        if !tool_errors.is_empty() {
            // For now, return the first tool error
            let (tool_name, error_message, arguments) = tool_errors.remove(0);
            return Ok(RemoteResponse::tool_error(
                "", // Will be replaced by caller
                &tool_name,
                &error_message,
                arguments,
            ));
        }
        
        Ok(RemoteResponse {
            request_id: String::new(), // Will be replaced by caller
            response: crate::remote::protocol::ResponseContent::Stream(response_chunks),
            error: None,
            token_usage,
        })
    }
    
    /// 处理聊天请求（带WebSocket交互）
    async fn process_chat_with_ws(
        &self,
        ws_stream: &mut WebSocketStream<TcpStream>,
        chat: &mut Chat,
        input: &str,
    ) -> Result<RemoteResponse> {
        // Use the shared function from the shared module
        use crate::remote::shared::process_streaming_chat_with_ws;
        process_streaming_chat_with_ws(ws_stream, chat, input).await
    }
}
