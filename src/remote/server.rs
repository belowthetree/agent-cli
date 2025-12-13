//! 用于处理远程连接的 WebSocket 服务器。

use crate::config::Config;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use log::{info, error};
use std::sync::Arc;

use super::client_handler::ClientHandler;

/// 用于处理远程客户端连接的 WebSocket 服务器。
pub struct RemoteServer {
    listener: TcpListener,
    config: Arc<Config>,
}

impl RemoteServer {
    /// 创建一个绑定到指定地址的新远程服务器。
    pub async fn new(addr: &str) -> anyhow::Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        info!("WebSocket server listening on {}", addr);
        
        let config = Arc::new(Config::local().map_err(|e| anyhow::anyhow!("Failed to load config: {}", e))?);
        
        Ok(Self { listener, config })
    }

    /// 运行服务器，无限期地接受连接。
    pub async fn run(&self) -> anyhow::Result<()> {
        info!("WebSocket server started");
        
        loop {
            match self.listener.accept().await {
                Ok((stream, addr)) => {
                    info!("Accepted connection from {}", addr);
                    
                    let config = Arc::clone(&self.config);
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_connection(stream, addr, config).await {
                            error!("Error handling connection from {}: {}", addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("Error accepting connection: {}", e);
                }
            }
        }
    }

    /// 处理单个客户端连接。
    async fn handle_connection(stream: tokio::net::TcpStream, addr: std::net::SocketAddr, config: Arc<Config>) -> anyhow::Result<()> {
        // 升级到 WebSocket 连接
        let ws_stream = match accept_async(stream).await {
            Ok(ws) => ws,
            Err(e) => {
                error!("WebSocket handshake failed for {}: {}", addr, e);
                return Err(anyhow::anyhow!("WebSocket handshake failed: {}", e));
            }
        };
        
        info!("WebSocket connection established with {}", addr);
        
        let config = (*config).clone();
        let mut handler = ClientHandler::new(ws_stream, config);
        handler.handle().await
    }
}
