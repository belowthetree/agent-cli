//! 用于处理单个客户端连接的客户端处理器。

use crate::chat::Chat;
use crate::config::Config;
use crate::mcp;
use futures::{SinkExt, StreamExt};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;
use tokio::net::TcpStream;
use log::{info, error, warn};

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
            return self.handle_instruction(&request.request_id, command, parameters).await;
        }
        
        // Handle Interrupt request
        if let InputType::Interrupt = &request.input {
            return self.handle_interrupt(&request.request_id);
        }
        
        // Handle Regenerate request
        if let InputType::Regenerate = &request.input {
            return self.handle_regenerate(&request.request_id).await;
        }
        
        // Handle ClearContext request
        if let InputType::ClearContext = &request.input {
            return self.handle_clear_context(&request.request_id).await;
        }
        
        // Handle ToolConfirmationResponse request
        if let InputType::ToolConfirmationResponse { name, arguments, approved, reason } = &request.input {
            return self.handle_tool_confirmation(&request.request_id, name, arguments, *approved, reason.as_deref()).await;
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
        let mut tool_errors = Vec::new();
        
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
        
        // Check if chat is waiting for tool confirmation
        if chat.is_waiting_tool_confirmation() {
            // Get the last tool call from context
            if let Some(last_msg) = chat.context.last() {
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
                            response: super::protocol::ResponseContent::ToolConfirmationRequest {
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
        let token_usage = chat.context.last().and_then(|last_msg| {
            last_msg.token_usage.as_ref().map(|usage| super::protocol::TokenUsage {
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
            if let Some(last_msg) = chat.context.last() {
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
                            response: super::protocol::ResponseContent::ToolConfirmationRequest {
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
        let token_usage = chat.context.last().and_then(|last_msg| {
            last_msg.token_usage.as_ref().map(|usage| super::protocol::TokenUsage {
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
        &mut self,
        request_id: &str,
        command: &str,
        parameters: &serde_json::Value,
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
        
        // 使用现有的聊天实例或创建新的
        let chat = match &mut self.chat {
            Some(chat) => {
                // 使用现有的聊天实例
                chat
            }
            None => {
                // 创建新的聊天实例
                let chat = Chat::new(self.config.clone());
                self.chat = Some(chat);
                self.chat.as_mut().unwrap()
            }
        };
        
        // 执行指令
        match cmd.execute(chat, parameters.clone()).await {
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

    /// 处理工具确认响应。
    async fn handle_tool_confirmation(
        &mut self,
        request_id: &str,
        tool_name: &str,
        arguments: &serde_json::Value,
        approved: bool,
        reason: Option<&str>,
    ) -> RemoteResponse {
        info!("Handling tool confirmation response: {} - tool: {}, approved: {}", 
            request_id, tool_name, approved);
        
        if let Some(chat) = &mut self.chat {
            if chat.is_waiting_tool_confirmation() {
                // 验证工具名称和参数是否匹配
                let mut validation_error = None;
                
                // 获取等待确认的工具调用
                if let Some(last_msg) = chat.context.last() {
                    if let Some(tool_calls) = &last_msg.tool_calls {
                        if let Some(tool_call) = tool_calls.first() {
                            // 验证工具名称
                            if tool_call.function.name != tool_name {
                                validation_error = Some(format!(
                                    "Tool name mismatch: expected '{}', got '{}'",
                                    tool_call.function.name, tool_name
                                ));
                            } else {
                                // 验证参数（可选，因为参数可能被序列化为字符串）
                                // 尝试解析工具调用中的参数
                                match serde_json::from_str::<serde_json::Value>(&tool_call.function.arguments) {
                                    Ok(expected_args) => {
                                        // 简单比较JSON值是否相等
                                        if &expected_args != arguments {
                                            warn!("Tool arguments mismatch for tool '{}'", tool_name);
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
                    return RemoteResponse::error(request_id, &format!("Tool confirmation validation failed: {}", error_msg));
                }
                
                // 设置工具确认结果
                chat.set_tool_confirmation_result(approved);
                
                // 如果有原因，记录下来
                if let Some(reason) = reason {
                    info!("Tool confirmation reason: {}", reason);
                }
                
                // 继续处理工具调用
                let result = if approved {
                    // 如果批准，继续执行工具
                    Self::process_non_streaming_chat(chat, "").await
                } else {
                    // 如果不批准，返回错误
                    Ok(RemoteResponse {
                        request_id: String::new(),
                        response: super::protocol::ResponseContent::Text(
                            format!("Tool '{}' execution was not approved by user", tool_name)
                        ),
                        error: None,
                        token_usage: None,
                    })
                };
                
                match result {
                    Ok(mut response) => {
                        response.request_id = request_id.to_string();
                        response
                    }
                    Err(e) => RemoteResponse::error(request_id, &format!("Tool confirmation processing error: {}", e)),
                }
            } else {
                RemoteResponse::error(request_id, "No pending tool confirmation found")
            }
        } else {
            RemoteResponse::error(request_id, "No active chat session found")
        }
    }

    /// 处理清理上下文请求。
    async fn handle_clear_context(&mut self, request_id: &str) -> RemoteResponse {
        info!("Handling clear context request: {}", request_id);
        
        if let Some(chat) = &mut self.chat {
            // 重置对话轮次
            chat.reset_conversation_turn();
            
            // 清理上下文（保留系统消息）
            let system_message = chat.context.first().cloned();
            chat.context.clear();
            
            if let Some(sys_msg) = system_message {
                chat.context.push(sys_msg);
            }
            
            info!("Chat context cleared successfully");
            
            RemoteResponse {
                request_id: request_id.to_string(),
                response: super::protocol::ResponseContent::Text("上下文已清理，对话轮次已重置".to_string()),
                error: None,
                token_usage: None,
            }
        } else {
            RemoteResponse::error(request_id, "No chat session found to clear context")
        }
    }
}
