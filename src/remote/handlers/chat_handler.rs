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
