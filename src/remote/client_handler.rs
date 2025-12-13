//! 用于处理单个客户端连接的客户端处理器。

use crate::chat::Chat;
use crate::config::Config;
use crate::mcp;
use futures::{SinkExt, StreamExt};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;
use tokio::net::TcpStream;
use log::{info, error};

use super::protocol::{RemoteRequest, RemoteResponse, InputType};
use super::commands::global_registry;

/// 用于处理单个客户端连接的处理器。
pub struct ClientHandler {
    ws_stream: WebSocketStream<TcpStream>,
    config: Config,
    chat: Option<Chat>,
}

impl ClientHandler {
    /// 创建一个新的客户端处理器。
    pub fn new(ws_stream: WebSocketStream<TcpStream>, config: Config) -> Self {
        Self { ws_stream, config, chat: None }
    }

    /// 处理客户端连接。
    pub async fn handle(&mut self) -> anyhow::Result<()> {
        info!("New WebSocket client connected");
        
        loop {
            match self.ws_stream.next().await {
                Some(Ok(message)) => {
                    match message {
                        Message::Text(text) => {
                            info!("Received WebSocket message: {}", text);
                            
                            // Parse the request
                            let request: RemoteRequest = match serde_json::from_str(&text) {
                                Ok(req) => req,
                                Err(e) => {
                                    error!("Failed to parse request: {} {}", e, text);
                                    let error_response = RemoteResponse::error("parse_error", &format!("Invalid request format: {}", e));
                                    let error_json = serde_json::to_string(&error_response)?;
                                    self.ws_stream.send(Message::Text(error_json)).await?;
                                    continue;
                                }
                            };

                            // Process the request
                            let response = self.process_request_internal(request).await;
                            
                            // Send the response
                            let response_json = serde_json::to_string(&response)?;
                            self.ws_stream.send(Message::Text(response_json)).await?;
                        }
                        Message::Binary(data) => {
                            info!("Received binary message ({} bytes)", data.len());
                            // Try to parse as JSON string
                            if let Ok(text) = String::from_utf8(data) {
                                match serde_json::from_str::<RemoteRequest>(&text) {
                                    Ok(request) => {
                                        let response = self.process_request_internal(request).await;
                                        let response_json = serde_json::to_string(&response)?;
                                        self.ws_stream.send(Message::Text(response_json)).await?;
                                    }
                                    Err(e) => {
                                        error!("Failed to parse binary message as JSON: {}", e);
                                        let error_response = RemoteResponse::error("parse_error", &format!("Invalid request format: {}", e));
                                        let error_json = serde_json::to_string(&error_response)?;
                                        self.ws_stream.send(Message::Text(error_json)).await?;
                                    }
                                }
                            } else {
                                error!("Binary message is not valid UTF-8");
                                let error_response = RemoteResponse::error("parse_error", "Binary message must be valid UTF-8 JSON");
                                let error_json = serde_json::to_string(&error_response)?;
                                self.ws_stream.send(Message::Text(error_json)).await?;
                            }
                        }
                        Message::Ping(data) => {
                            info!("Received ping, sending pong");
                            self.ws_stream.send(Message::Pong(data)).await?;
                        }
                        Message::Pong(_) => {
                            // Ignore pong messages
                        }
                        Message::Close(frame) => {
                            info!("Received close frame: {:?}", frame);
                            if let Some(frame) = frame {
                                self.ws_stream.send(Message::Close(Some(frame))).await?;
                            } else {
                                self.ws_stream.send(Message::Close(None)).await?;
                            }
                            break;
                        }
                        Message::Frame(_) => {
                            // Raw frame, we don't handle this directly
                            // It's handled internally by tungstenite
                        }
                    }
                }
                Some(Err(e)) => {
                    error!("WebSocket error: {}", e);
                    break;
                }
                None => {
                    info!("WebSocket connection closed by client");
                    break;
                }
            }
        }

        Ok(())
    }

    /// 处理单个请求（内部辅助函数，用于避免借用问题）。
    async fn process_request_internal(&mut self, request: RemoteRequest) -> RemoteResponse {
        info!("Processing request: {}", request.request_id);
        
        // Handle GetCommands request
        if let InputType::GetCommands = &request.input {
            return Self::handle_get_commands(&request.request_id);
        }
        
        // Handle Instruction request
        if let InputType::Instruction { command, parameters } = &request.input {
            return Self::handle_instruction(&request.request_id, command, parameters, &self.config).await;
        }
        
        // Handle Interrupt request
        if let InputType::Interrupt = &request.input {
            return self.handle_interrupt(&request.request_id);
        }
        
        // Handle Regenerate request
        if let InputType::Regenerate = &request.input {
            return self.handle_regenerate(&request.request_id).await;
        }
        
        // Extract text from input
        let input_text = request.input.to_text();
        
        // Merge request config with default config
        let mut chat_config = self.config.clone();
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

        // Get or create chat instance
        let chat = match &mut self.chat {
            Some(chat) => {
                // Chat already exists, use it
                // Note: We don't update config for existing chat
                chat
            }
            None => {
                let mut chat = Chat::new(chat_config);
                
                // Configure tools if requested
                let use_tools = request.use_tools.unwrap_or(true);
                if use_tools {
                    chat = chat.tools(mcp::get_config_tools());
                    chat = chat.tools(mcp::get_basic_tools());
                }
                
                self.chat = Some(chat);
                self.chat.as_mut().unwrap()
            }
        };

        // Determine streaming mode
        let stream_mode = request.stream.unwrap_or(false);
        
        let result = if stream_mode {
            Self::process_streaming_chat(chat, &input_text).await
        } else {
            Self::process_non_streaming_chat(chat, &input_text).await
        };

        match result {
            Ok(mut response) => {
                // Set the correct request ID
                response.request_id = request.request_id;
                response
            }
            Err(e) => RemoteResponse::error(&request.request_id, &format!("Processing error: {}", e)),
        }
    }

    /// 处理非流式聊天请求。
    async fn process_non_streaming_chat(
        chat: &mut Chat,
        input: &str,
    ) -> anyhow::Result<RemoteResponse> {
        use futures::StreamExt;
        
        let mut response_text = String::new();
        
        // Consume the stream in an inner scope to ensure it's dropped before accessing chat.context
        {
            let stream = chat.chat(input);
            futures::pin_mut!(stream);
            
            while let Some(result) = stream.next().await {
                match result {
                    Ok(response) => {
                        use crate::chat::StreamedChatResponse;
                        match response {
                            StreamedChatResponse::Text(text) => {
                                response_text.push_str(&text);
                            }
                            StreamedChatResponse::Reasoning(think) => {
                                // Optionally include reasoning in response
                                if !think.is_empty() {
                                    response_text.push_str(&format!("[Reasoning: {}] ", think));
                                }
                            }
                            StreamedChatResponse::ToolCall(tool_call) => {
                                response_text.push_str(&format!("[Tool call: {}] ", tool_call.function.name));
                            }
                            StreamedChatResponse::ToolResponse(tool_response) => {
                                if !tool_response.content.is_empty() {
                                    response_text.push_str(&format!("[Tool result: {}] ", tool_response.content));
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
        
        // Get token usage from last message if available (after stream is consumed and dropped)
        let token_usage = chat.context.last().and_then(|last_msg| {
            last_msg.token_usage.as_ref().map(|usage| super::protocol::TokenUsage {
                prompt_tokens: usage.prompt_tokens,
                completion_tokens: usage.completion_tokens,
                total_tokens: usage.total_tokens,
            })
        });
        
        Ok(RemoteResponse {
            request_id: String::new(), // Will be replaced by caller
            response: super::protocol::ResponseContent::Text(response_text),
            error: None,
            token_usage,
        })
    }

    /// 处理流式聊天请求。
    async fn process_streaming_chat(
        chat: &mut Chat,
        input: &str,
    ) -> anyhow::Result<RemoteResponse> {
        use futures::StreamExt;
        
        let mut response_chunks = Vec::new();
        
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
        
        // Get token usage from last message if available (after stream is consumed and dropped)
        let token_usage = chat.context.last().and_then(|last_msg| {
            last_msg.token_usage.as_ref().map(|usage| super::protocol::TokenUsage {
                prompt_tokens: usage.prompt_tokens,
                completion_tokens: usage.completion_tokens,
                total_tokens: usage.total_tokens,
            })
        });
        
        Ok(RemoteResponse {
            request_id: String::new(), // Will be replaced by caller
            response: super::protocol::ResponseContent::Stream(response_chunks),
            error: None,
            token_usage,
        })
    }

    /// 处理获取内置指令列表的请求。
    fn handle_get_commands(request_id: &str) -> RemoteResponse {
        info!("Handling GetCommands request: {}", request_id);
        
        // 获取全局指令注册器
        let registry = global_registry();
        
        // 构建命令列表
        let mut commands_list = Vec::new();
        for cmd in registry.all() {
            commands_list.push(serde_json::json!({
                "name": cmd.name(),
                "description": cmd.description(),
            }));
        }
        
        // 创建响应
        let response_json = serde_json::json!({
            "commands": commands_list,
            "count": commands_list.len(),
        });
        
        RemoteResponse {
            request_id: request_id.to_string(),
            response: super::protocol::ResponseContent::Text(
                serde_json::to_string(&response_json).unwrap_or_else(|_| "{\"error\": \"Failed to serialize commands\"}".to_string())
            ),
            error: None,
            token_usage: None,
        }
    }

    /// 处理指令请求。
    async fn handle_instruction(
        request_id: &str,
        command: &str,
        parameters: &serde_json::Value,
        base_config: &Config,
    ) -> RemoteResponse {
        info!("Handling instruction request: {} - command: {}", request_id, command);
        
        // 获取全局指令注册器
        let registry = global_registry();
        
        // 查找指令
        let cmd = match registry.find(command) {
            Some(cmd) => cmd,
            None => {
                return RemoteResponse::error(
                    request_id,
                    &format!("Unknown command: {}", command)
                );
            }
        };
        
        // 创建聊天实例
        let mut chat = Chat::new(base_config.clone());
        
        // 执行指令
        match cmd.execute(&mut chat, parameters.clone()).await {
            Ok(result) => {
                RemoteResponse {
                    request_id: request_id.to_string(),
                    response: super::protocol::ResponseContent::Text(result),
                    error: None,
                    token_usage: None,
                }
            }
            Err(error_msg) => {
                RemoteResponse::error(request_id, &format!("Command execution failed: {}", error_msg))
            }
        }
    }

    /// 处理中断请求。
    fn handle_interrupt(&mut self, request_id: &str) -> RemoteResponse {
        info!("Handling interrupt request: {}", request_id);
        
        if let Some(chat) = &self.chat {
            if chat.is_running() {
                chat.cancel();
                info!("Chat interrupted successfully");
                return RemoteResponse {
                    request_id: request_id.to_string(),
                    response: super::protocol::ResponseContent::Text("Model output interrupted successfully".to_string()),
                    error: None,
                    token_usage: None,
                };
            } else {
                return RemoteResponse::error(request_id, "No active model output to interrupt");
            }
        } else {
            return RemoteResponse::error(request_id, "No chat session found to interrupt");
        }
    }

    /// 处理重新生成请求。
    async fn handle_regenerate(&mut self, request_id: &str) -> RemoteResponse {
        info!("Handling regenerate request: {}", request_id);
        
        if let Some(chat) = &mut self.chat {
            if !chat.is_running() {
                // 使用stream_rechat重新生成回复
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
                                    StreamedChatResponse::End => {
                                        // End marker, do nothing here
                                    }
                                }
                            }
                            Err(e) => {
                                return RemoteResponse::error(request_id, &format!("Regeneration error: {}", e));
                            }
                        }
                    }
                }
                
                RemoteResponse {
                    request_id: request_id.to_string(),
                    response: super::protocol::ResponseContent::Stream(response_chunks),
                    error: None,
                    token_usage: None,
                }
            } else {
                RemoteResponse::error(request_id, "Cannot regenerate while model is running")
            }
        } else {
            RemoteResponse::error(request_id, "No chat session found to regenerate")
        }
    }
}
