use crate::acp::types::{AcpError, AcpResult};
use crate::acp::transport::Transport;
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use std::pin::Pin;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

/// WebSocket传输层实现
pub struct WsTransport {
    addr: String,
    port: u16,
    sender: mpsc::UnboundedSender<String>,
}

impl WsTransport {
    pub fn new(addr: &str, port: u16) -> Self {
        let (sender, _) = mpsc::unbounded_channel();
        Self {
            addr: addr.to_string(),
            port,
            sender,
        }
    }
}

#[async_trait]
impl Transport for WsTransport {
    async fn send_response(&self, response: &str) -> AcpResult<()> {
        self.send_notification(response).await
    }

    async fn send_notification(&self, notification: &str) -> AcpResult<()> {
        if let Err(e) = self.sender.send(notification.to_string()) {
            return Err(AcpError::TransportError(format!("发送消息失败: {}", e)));
        }
        Ok(())
    }

    fn receive_stream(&self) -> Pin<Box<dyn Stream<Item = Result<String, AcpError>> + Send + '_>> {
        let addr = self.addr.clone();
        let port = self.port;
        
        Box::pin(async_stream::try_stream! {
            let url = format!("ws://{}:{}", addr, port);
            let (ws_stream, _) = tokio_tungstenite::connect_async(&url).await
                .map_err(|e| AcpError::TransportError(format!("WebSocket连接失败: {}", e)))?;
            
            let (_write, mut read) = ws_stream.split();
            
            // 处理接收
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        yield text;
                    }
                    Ok(Message::Close(_)) => {
                        break;
                    }
                    Err(e) => {
                        Err(AcpError::TransportError(format!("WebSocket读取错误: {}", e)))?;
                        break;
                    }
                    _ => {}
                }
            }
        })
    }

    async fn close(&self) -> AcpResult<()> {
        Ok(())
    }
}
