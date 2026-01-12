use crate::acp::types::{AcpError, AcpResult};
use crate::acp::transport::Transport;
use async_trait::async_trait;
use eventsource_client as es;
use eventsource_client::Client;
use futures::{Stream, StreamExt};
use std::pin::Pin;
use tokio::sync::mpsc;

/// HTTP传输层实现 (使用SSE)
pub struct HttpTransport {
    addr: String,
    port: u16,
    sender: mpsc::UnboundedSender<String>,
}

impl HttpTransport {
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
impl Transport for HttpTransport {
    async fn send_response(&self, response: &str) -> AcpResult<()> {
        self.send_notification(response).await
    }

    async fn send_notification(&self, notification: &str) -> AcpResult<()> {
        // HTTP通过POST发送消息
        let url = format!("http://{}:{}/acp/message", self.addr, self.port);
        
        let client = reqwest::Client::new();
        let res = client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(notification.to_string())
            .send()
            .await
            .map_err(|e| AcpError::TransportError(format!("HTTP发送失败: {}", e)))?;

        if !res.status().is_success() {
            return Err(AcpError::TransportError(format!(
                "HTTP错误: {}",
                res.status()
            )));
        }

        Ok(())
    }

    fn receive_stream(&self) -> Pin<Box<dyn Stream<Item = Result<String, AcpError>> + Send + '_>> {
        let addr = self.addr.clone();
        let port = self.port;
        
        Box::pin(async_stream::try_stream! {
            let url = format!("http://{}:{}/acp/stream", addr, port);
            
            let client = es::ClientBuilder::for_url(&url)
                .map_err(|e| AcpError::TransportError(format!("创建SSE客户端失败: {}", e)))?
                .build();

            let mut stream = client.stream();

            while let Some(event) = stream.next().await {
                match event {
                    Ok(es::SSE::Event(event)) => {
                        yield event.data;
                    }
                    Ok(es::SSE::Comment(_)) => {
                        // 忽略注释
                    }
                    Ok(es::SSE::Connected(_)) => {
                        // 忽略连接消息
                    }
                    Err(e) => {
                        Err(AcpError::TransportError(format!("SSE读取错误: {}", e)))?;
                        break;
                    }
                }
            }
        })
    }

    async fn close(&self) -> AcpResult<()> {
        Ok(())
    }
}
