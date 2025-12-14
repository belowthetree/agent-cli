//! 处理 ClearContext 请求的处理器

use super::base_handler::RequestHandler;
use crate::remote::protocol::{RemoteRequest, RemoteResponse};
use crate::config::Config;
use crate::chat::Chat;
use tokio_tungstenite::WebSocketStream;
use tokio::net::TcpStream;
use log::info;

/// 处理 ClearContext 请求的处理器
pub struct ClearContextHandler;

#[async_trait::async_trait]
impl RequestHandler for ClearContextHandler {
    async fn handle(
        &self,
        request: RemoteRequest,
        chat: Option<&mut Chat>,
        _config: &Config,
        _ws_stream: Option<&mut WebSocketStream<TcpStream>>,
    ) -> RemoteResponse {
        info!("Handling clear context request: {}", request.request_id);
        
        if let Some(chat) = chat {
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
                request_id: request.request_id,
                response: crate::remote::protocol::ResponseContent::Text("上下文已清理，对话轮次已重置".to_string()),
                error: None,
                token_usage: None,
            }
        } else {
            RemoteResponse::error(&request.request_id, "No chat session found to clear context")
        }
    }
    
    fn can_handle(&self, request: &RemoteRequest) -> bool {
        matches!(&request.input, crate::remote::protocol::InputType::ClearContext)
    }
}
