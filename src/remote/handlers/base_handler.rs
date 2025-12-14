//! 基础处理器 trait

use crate::remote::protocol::{RemoteRequest, RemoteResponse};
use crate::config::Config;
use crate::chat::Chat;
use tokio_tungstenite::WebSocketStream;
use tokio::net::TcpStream;

/// 请求处理器 trait
#[async_trait::async_trait]
pub trait RequestHandler: Send + Sync {
    /// 处理请求
    async fn handle(
        &self,
        request: RemoteRequest,
        chat: Option<&mut Chat>,
        config: &Config,
        ws_stream: Option<&mut WebSocketStream<TcpStream>>,
    ) -> RemoteResponse;
    
    /// 检查此处理器是否可以处理给定的请求
    #[allow(unused)]
    fn can_handle(&self, request: &RemoteRequest) -> bool;
}
