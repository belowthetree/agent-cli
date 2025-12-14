//! 处理 Interrupt 请求的处理器

use super::base_handler::RequestHandler;
use crate::remote::protocol::{RemoteRequest, RemoteResponse};
use crate::config::Config;
use crate::chat::Chat;
use tokio_tungstenite::WebSocketStream;
use tokio::net::TcpStream;
use log::info;

/// 处理 Interrupt 请求的处理器
pub struct InterruptHandler;

#[async_trait::async_trait]
impl RequestHandler for InterruptHandler {
    async fn handle(
        &self,
        request: RemoteRequest,
        chat: &mut Chat,
        _config: &Config,
        _ws_stream: &mut WebSocketStream<TcpStream>,
    ) -> RemoteResponse {
        info!("Handling interrupt request: {}", request.request_id);
        if chat.is_running() {
            chat.cancel();
            info!("Chat interrupted successfully");
            return RemoteResponse {
                request_id: request.request_id,
                response: crate::remote::protocol::ResponseContent::Text("Model output interrupted successfully".to_string()),
                error: None,
                token_usage: None,
            };
        } else {
            return RemoteResponse::error(&request.request_id, "No active model output to interrupt");
        }
    }
    
    fn can_handle(&self, request: &RemoteRequest) -> bool {
        matches!(&request.input, crate::remote::protocol::InputType::Interrupt)
    }
}
