//! 共享工具和辅助函数模块

use crate::chat::Chat;
use crate::remote::protocol::{RemoteResponse, ResponseContent, TokenUsage, RemoteRequest, InputType};
use futures::{SinkExt, StreamExt};
use log::{info, warn, error};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;
use tokio::net::TcpStream;
use anyhow::Result;

/// 处理流式聊天响应的共享函数
pub async fn process_streaming_chat_with_ws(
    ws_stream: &mut WebSocketStream<TcpStream>,
    chat: &mut Chat,
    input: &str,
) -> Result<RemoteResponse> {
    let mut response_chunks = Vec::new();
    let mut tool_errors = Vec::new();
    
    // Get a copy of the cancel token before creating the stream
    let cancel_token = chat.get_cancel_token();
    
    // Consume the stream in an inner scope to ensure it's dropped before accessing chat.context
    {
        let stream = chat.stream_chat(input);
        futures::pin_mut!(stream);
        
        loop {
            // Use tokio::select! to concurrently wait for stream items and WebSocket messages
            tokio::select! {
                result = stream.next() => {
                    match result {
                        Some(result) => {
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
                                            // End marker, break the loop
                                            break;
                                        }
                                    }
                                }
                                Err(e) => {
                                    return Err(anyhow::anyhow!("Chat error: {}", e));
                                }
                            }
                        }
                        None => {
                            // Stream ended
                            break;
                        }
                    }
                }
                // Check for WebSocket messages (interrupt requests)
                ws_message = ws_stream.next() => {
                    match ws_message {
                        Some(Ok(message)) => {
                            match message {
                                Message::Text(text) => {
                                    info!("Received WebSocket message during streaming chat processing: {}", text);
                                    
                                    // Try to parse as interrupt request
                                    if let Ok(request) = serde_json::from_str::<RemoteRequest>(&text) {
                                        if let InputType::Interrupt = &request.input {
                                            // Handle interrupt immediately
                                            cancel_token.cancel();
                                            info!("Streaming chat interrupted during processing");
                                            // Return a response indicating interruption
                                            return Ok(RemoteResponse {
                                                request_id: String::new(), // Will be replaced by caller
                                                response: ResponseContent::Text("Model output interrupted by user request".to_string()),
                                                error: None,
                                                token_usage: None,
                                            });
                                        }
                                    }
                                    // If not an interrupt, ignore for now (could queue it for later)
                                }
                                Message::Binary(data) => {
                                    // Try to parse binary as JSON string
                                    if let Ok(text) = String::from_utf8(data) {
                                        info!("Received binary WebSocket message during streaming chat processing: {}", text);
                                        
                                        if let Ok(request) = serde_json::from_str::<RemoteRequest>(&text) {
                                            if let InputType::Interrupt = &request.input {
                                                // Handle interrupt immediately
                                                cancel_token.cancel();
                                                info!("Streaming chat interrupted during processing");
                                                // Return a response indicating interruption
                                                return Ok(RemoteResponse {
                                                    request_id: String::new(), // Will be replaced by caller
                                                    response: ResponseContent::Text("Model output interrupted by user request".to_string()),
                                                    error: None,
                                                    token_usage: None,
                                                });
                                            }
                                        }
                                    }
                                }
                                Message::Ping(data) => {
                                    // Respond to ping
                                    let _ = ws_stream.send(Message::Pong(data)).await;
                                }
                                Message::Pong(_) => {
                                    // Ignore pong
                                }
                                Message::Close(_frame) => {
                                    // Connection closed, cancel chat
                                    cancel_token.cancel();
                                    info!("WebSocket closed during streaming chat processing");
                                    return Err(anyhow::anyhow!("WebSocket connection closed during streaming chat processing"));
                                }
                                Message::Frame(_) => {
                                    // Ignore raw frames
                                }
                            }
                        }
                        Some(Err(e)) => {
                            error!("WebSocket error during streaming chat processing: {}", e);
                            cancel_token.cancel();
                            return Err(anyhow::anyhow!("WebSocket error: {}", e));
                        }
                        None => {
                            // WebSocket closed
                            cancel_token.cancel();
                            info!("WebSocket connection closed during streaming chat processing");
                            return Err(anyhow::anyhow!("WebSocket connection closed"));
                        }
                    }
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
                            warn!("Failed to parse tool arguments as JSON: {}", e);
                            serde_json::json!({})
                        }
                    };
                    
                    // Return a tool confirmation request
                    return Ok(RemoteResponse {
                        request_id: String::new(), // Will be replaced by caller
                        response: ResponseContent::ToolConfirmationRequest {
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
    
    // Get token usage from last message if available (after stream is consumed and dropped)
    let token_usage = chat.context().last().and_then(|last_msg| {
        last_msg.token_usage.as_ref().map(|usage| TokenUsage {
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
        response: ResponseContent::Stream(response_chunks),
        error: None,
        token_usage,
    })
}
