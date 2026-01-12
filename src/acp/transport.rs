use crate::acp::types::{AcpError, AcpResult};
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

pub mod stdio_transport;
pub mod ws_transport;
pub mod http_transport;

pub use stdio_transport::StdioTransport;
pub use ws_transport::WsTransport;
pub use http_transport::HttpTransport;

/// 传输配置
#[derive(Debug, Clone)]
pub struct TransportConfig {
    pub transport_type: TransportType,
    pub addr: Option<String>,
    pub port: Option<u16>,
}

/// 传输类型
#[derive(Debug, Clone, PartialEq)]
pub enum TransportType {
    Stdio,
    WebSocket,
    Http,
}

/// 传输层trait
#[async_trait]
pub trait Transport: Send + Sync {
    /// 发送响应
    async fn send_response(&self, response: &str) -> AcpResult<()>;

    /// 发送通知
    async fn send_notification(&self, notification: &str) -> AcpResult<()>;

    /// 接收消息流
    fn receive_stream(&self) -> Pin<Box<dyn Stream<Item = Result<String, AcpError>> + Send + '_>>;

    /// 关闭连接
    async fn close(&self) -> AcpResult<()>;
}

/// 根据配置创建传输层
pub fn create_transport(config: TransportConfig) -> AcpResult<Box<dyn Transport>> {
    match config.transport_type {
        TransportType::Stdio => Ok(Box::new(StdioTransport::new())),
        TransportType::WebSocket => {
            let addr = config.addr.ok_or_else(|| {
                AcpError::TransportError("WebSocket需要地址".to_string())
            })?;
            let port = config.port.ok_or_else(|| {
                AcpError::TransportError("WebSocket需要端口".to_string())
            })?;
            Ok(Box::new(WsTransport::new(&addr, port)))
        }
        TransportType::Http => {
            let addr = config.addr.ok_or_else(|| {
                AcpError::TransportError("HTTP需要地址".to_string())
            })?;
            let port = config.port.ok_or_else(|| {
                AcpError::TransportError("HTTP需要端口".to_string())
            })?;
            Ok(Box::new(HttpTransport::new(&addr, port)))
        }
    }
}
