//! 共享工具和辅助函数模块

use crate::chat::Chat;
use crate::chat::EChatState;
use crate::chat::StreamedChatResponse;
use crate::remote::protocol::{RemoteResponse, ResponseContent, TokenUsage, RemoteRequest, InputType};
use futures::{SinkExt, StreamExt};
use log::{info, warn, error};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use anyhow::Result;

/// 处理流式聊天响应的共享函数
pub async fn process_streaming_chat_with_ws(
    ws_stream: &mut WebSocketStream<TcpStream>,
    chat: &mut Chat,
    input: &str,
    request_id: &str,
) -> Result<RemoteResponse> {
    let mut tool_errors = Vec::new();
    let cancel_token = chat.get_cancel_token();
    
    // 创建一个通道来接收聊天流的结果
    let (tx, mut rx) = mpsc::channel::<Result<StreamedChatResponse, anyhow::Error>>(32);

    let input_clone = input.to_string();
    let mut chat_clone = chat.clone();
    
    // 创建一个单独的任务来处理聊天流
    let chat_task = tokio::spawn(async move {
        {
            let stream = chat_clone.stream_chat(&input_clone);
            futures::pin_mut!(stream);
            
            while let Some(result) = stream.next().await {
                // 发送结果到通道
                if tx.send(result).await.is_err() {
                    // 接收端已关闭，退出任务
                    break;
                }
            }
        }
        info!("聊天流任务完成");
        chat_clone
    });
    
    // 使用 tokio::select! 同时监听聊天流结果和 WebSocket 消息
    let mut chat_task_completed = false;
    let mut interrupted = false;
    
    loop {
        tokio::select! {
            // 从通道接收聊天流结果
            result = rx.recv() => {
                match result {
                    Some(res) => {
                        match res {
                            Ok(res) => {
                                match res {
                                    // 判断是否有工具错误
                                    StreamedChatResponse::ToolResponse(res) => {
                                        if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&res.content) {
                                            if let Some(error_field) = json_value.get("error") {
                                                if error_field == true {
                                                    tool_errors.push((res.name, res.content.to_string()));
                                                }
                                            }
                                        }
                                    }
                                    msg => {
                                        if let Ok(chunk_response) = RemoteResponse::model_message(msg.clone(), request_id.to_string()) {
                                            if let Ok(json) = serde_json::to_string(&chunk_response) {
                                                let _ = ws_stream.send(Message::Text(json)).await;
                                            }
                                        } else {
                                            // 如果发送失败，记录错误但不立即返回
                                            error!("生成消息错误 {:?}", msg);
                                            break;
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                error!("聊天流错误: {}", e);
                                // 继续处理，不立即返回
                            }
                        }
                    }
                    None => {
                        // 通道关闭，聊天流任务完成
                        chat_task_completed = true;
                        break;
                    }
                }
            }
            // 检查 WebSocket 消息（包括 interrupt 请求）
            ws_message = ws_stream.next() => {
                match ws_message {
                    Some(Ok(message)) => {
                        match message {
                            Message::Text(text) => {
                                info!("Received WebSocket message during streaming chat processing: {}", text);
                                // 尝试解析为 interrupt 请求
                                if let Ok(request) = serde_json::from_str::<RemoteRequest>(&text) {
                                    if let InputType::Interrupt = &request.input {
                                        // 收到 interrupt，但不立即取消聊天流
                                        // 设置 interrupted 标志，让聊天流继续完成
                                        interrupted = true;
                                        cancel_token.cancel();
                                        info!("收到 interrupt 请求");
                                        // 不立即返回，继续处理聊天流
                                    }
                                }
                                // 如果不是 interrupt，忽略（可以排队稍后处理）
                            }
                            Message::Binary(data) => {
                                // 尝试将二进制数据解析为 JSON 字符串
                                if let Ok(text) = String::from_utf8(data) {
                                    info!("Received binary WebSocket message during streaming chat processing: {}", text);
                                    
                                    if let Ok(request) = serde_json::from_str::<RemoteRequest>(&text) {
                                        if let InputType::Interrupt = &request.input {
                                            // 收到 interrupt，但不立即取消聊天流
                                            interrupted = true;
                                            cancel_token.cancel();
                                            info!("收到 interrupt 请求");
                                            // 不立即返回，继续处理聊天流
                                        }
                                    }
                                }
                            }
                            Message::Ping(data) => {
                                // 响应 ping
                                let _ = ws_stream.send(Message::Pong(data)).await;
                            }
                            Message::Pong(_) => {
                                // 忽略 pong
                            }
                            Message::Close(_frame) => {
                                // 连接关闭，取消聊天
                                cancel_token.cancel();
                                info!("WebSocket closed during streaming chat processing");
                                return Err(anyhow::anyhow!("WebSocket connection closed during streaming chat processing"));
                            }
                            Message::Frame(_) => {
                                // 忽略原始帧
                            }
                        }
                    }
                    Some(Err(e)) => {
                        error!("WebSocket error during streaming chat processing: {}", e);
                        cancel_token.cancel();
                        return Err(anyhow::anyhow!("WebSocket error: {}", e));
                    }
                    None => {
                        // WebSocket 关闭
                        cancel_token.cancel();
                        info!("WebSocket connection closed during streaming chat processing");
                        return Err(anyhow::anyhow!("WebSocket connection closed"));
                    }
                }
            }
        }
        
        // 如果聊天任务已完成，退出循环
        if chat_task_completed {
            break;
        }
    }
    
    // 等待聊天任务完成（如果还没有完成）
    let returned_chat = if !chat_task_completed {
        chat_task.await.ok()
    } else {
        None
    };
    
    // 如果任务返回了修改后的 chat_clone，将其赋值回原始 chat
    if let Some(returned_chat) = returned_chat {
        *chat = returned_chat;
    }
    
    // 发送工具确认协议
    if chat.get_state() == EChatState::WaitingToolConfirm {
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
    
    // token 使用情况
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
        let (tool_name, error_message) = tool_errors.remove(0);
        return Ok(RemoteResponse::tool_error(
            "", // Will be replaced by caller
            &tool_name,
            &error_message,
            None,
        ));
    }

    // 返回流完成响应，表示流已结束
    // 所有响应块都已经实时发送给客户端
    Ok(RemoteResponse {
        request_id: String::new(), // Will be replaced by caller
        response: ResponseContent::StreamComplete {
            token_usage,
            interrupted,
        },
        error: None,
        token_usage: None, // token_usage 已经在 StreamComplete 中包含了
    })
}
