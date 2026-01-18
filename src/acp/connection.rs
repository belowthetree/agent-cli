//! ACP 连接模块
//!
//! 支持多种传输方式：stdio 和 wss (WebSocket Secure)

use agent_client_protocol::{self as acp, Client};
use anyhow::Result;
use async_trait::async_trait;
use futures::{SinkExt, StreamExt};
use log::{debug, error, info};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::task::LocalSet;
use tokio_tungstenite::{WebSocketStream, accept_async, tungstenite::Message};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

use crate::acp::agent_impl::AcpAgent;
use crate::config::Config;

/// 连接类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionType {
    Stdio,
    Wss,
}

/// 连接配置
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    pub connection_type: ConnectionType,
    pub wss_port: Option<u16>,
    pub server_name: String,
    pub server_version: String,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            connection_type: ConnectionType::Stdio,
            wss_port: None,
            server_name: "agent-cli".to_string(),
            server_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

/// 连接 trait
#[async_trait(?Send)]
pub trait AcpConnection {
    /// 运行连接
    async fn run(&self, config: Config) -> Result<()>;
}

/// Stdio 连接实现
pub struct StdioConnection {
    config: ConnectionConfig,
}

impl StdioConnection {
    pub fn new(config: ConnectionConfig) -> Self {
        Self { config }
    }
}

#[async_trait(?Send)]
impl AcpConnection for StdioConnection {
    async fn run(&self, config: Config) -> Result<()> {
        info!("启动 ACP Agent (stdio 模式)");

        // 创建会话更新通道
        let (session_update_tx, mut session_update_rx) = mpsc::unbounded_channel();

        let agent = AcpAgent::new(
            self.config.server_name.clone(),
            self.config.server_version.clone(),
            config,
            session_update_tx,
        );

        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();

        // 使用 LocalSet 来运行非 Send 的 future
        let local_set = LocalSet::new();

        local_set
            .run_until(async move {
                // 创建连接
                let (conn, handle_io) = acp::AgentSideConnection::new(
                    agent,
                    stdout.compat_write(), // outgoing: 写入响应到 stdout
                    stdin.compat(),        // incoming: 从 stdin 读取请求
                    |fut| {
                        tokio::task::spawn_local(fut);
                    },
                );

                // 克隆 conn 用于后台任务
                let conn_clone = conn;

                // 启动后台任务处理会话通知
                tokio::task::spawn_local(async move {
                    info!("启动会话通知处理任务");

                    while let Some((session_notification, tx)) = session_update_rx.recv().await {
                        debug!("发送会话通知: {:?}", session_notification);

                        match conn_clone.session_notification(session_notification).await {
                            Ok(_) => {
                                // 通知发送完成
                                tx.send(()).ok();
                            }
                            Err(e) => {
                                error!("发送会话通知失败: {}", e);
                                tx.send(()).ok();
                                break;
                            }
                        }
                    }

                    info!("会话通知处理任务结束");
                });

                // 在另一个任务中处理 I/O
                tokio::task::spawn_local(async move {
                    if let Err(e) = handle_io.await {
                        error!("ACP Agent I/O 错误: {}", e);
                    }
                });

                // 等待 Ctrl+C 信号
                tokio::signal::ctrl_c().await?;
                info!("收到 Ctrl+C 信号，退出 ACP Agent");

                Ok::<(), anyhow::Error>(())
            })
            .await?;

        Ok(())
    }
}

/// WSS 连接实现
pub struct WssConnection {
    config: ConnectionConfig,
}

impl WssConnection {
    pub fn new(config: ConnectionConfig) -> Self {
        Self { config }
    }

    /// 处理单个 WebSocket 连接
    async fn handle_connection(
        &self,
        stream: TcpStream,
        config: Config,
        server_name: String,
        server_version: String,
    ) -> Result<()> {
        info!("新的 WebSocket 连接");

        // 接受 WebSocket 连接
        let ws_stream = match accept_async(stream).await {
            Ok(stream) => stream,
            Err(e) => {
                error!("WebSocket 握手失败: {}", e);
                return Err(anyhow::anyhow!("WebSocket 握手失败: {}", e));
            }
        };

        info!("WebSocket 连接已建立");

        // 处理 WebSocket 连接
        self.process_websocket(ws_stream, config, server_name, server_version)
            .await
    }

    /// 处理 WebSocket 消息流
    async fn process_websocket(
        &self,
        ws_stream: WebSocketStream<TcpStream>,
        config: Config,
        server_name: String,
        server_version: String,
    ) -> Result<()> {
        let (mut ws_sender, mut ws_receiver) = ws_stream.split();

        // 创建会话更新通道
        let (session_update_tx, mut session_update_rx) = mpsc::unbounded_channel();

        // 创建 ACP Agent
        let agent = AcpAgent::new(server_name, server_version, config, session_update_tx);

        // 创建双向通道来桥接 WebSocket 和 ACP
        let (acp_to_ws_tx, mut acp_to_ws_rx) = mpsc::unbounded_channel::<String>();
        let (ws_to_acp_tx, ws_to_acp_rx) = mpsc::unbounded_channel::<String>();

        // 启动从 ACP 到 WebSocket 的转发任务
        let acp_to_ws_task = tokio::task::spawn_local(async move {
            while let Some(message) = acp_to_ws_rx.recv().await {
                info!("{:?}", message);
                if let Err(e) = ws_sender.send(Message::Text(message)).await {
                    error!("发送 WebSocket 消息失败: {}", e);
                    break;
                }
            }
        });

        // 启动从 WebSocket 到 ACP 的转发任务
        let ws_to_acp_task = tokio::task::spawn_local(async move {
            while let Some(message) = ws_receiver.next().await {
                info!("{:?}", message);
                match message {
                    Ok(Message::Text(text)) => {
                        if let Err(e) = ws_to_acp_tx.send(text) {
                            error!("转发到 ACP 失败: {}", e);
                            break;
                        }
                    }
                    Ok(Message::Close(_)) => {
                        break;
                    }
                    Err(e) => {
                        error!("接收 WebSocket 消息失败: {}", e);
                        break;
                    }
                    _ => {
                        info!("其他类型的消息");
                    }
                }
            }
        });

        // 创建自定义的读写器
        let outgoing = UnboundedSenderWriter::new(acp_to_ws_tx);
        let incoming = UnboundedReceiverReader::new(ws_to_acp_rx);

        // 创建 ACP 连接
        let (conn, handle_io) = acp::AgentSideConnection::new(agent, outgoing, incoming, |fut| {
            tokio::task::spawn_local(fut);
        });

        // 克隆 conn 用于后台任务
        let conn_clone = conn;

        // 启动后台任务处理会话通知
        let session_task = tokio::task::spawn_local(async move {
            info!("启动 WebSocket 会话通知处理任务");

            while let Some((session_notification, tx)) = session_update_rx.recv().await {
                debug!("发送会话通知: {:?}", session_notification);

                match conn_clone.session_notification(session_notification).await {
                    Ok(_) => {
                        // 通知发送完成
                        tx.send(()).ok();
                    }
                    Err(e) => {
                        error!("发送会话通知失败: {}", e);
                        tx.send(()).ok();
                        break;
                    }
                }
            }

            info!("WebSocket 会话通知处理任务结束");
        });

        // 处理 I/O
        let io_result = handle_io.await;

        // 等待所有任务完成
        acp_to_ws_task.await.ok();
        ws_to_acp_task.await.ok();
        session_task.await.ok();

        if let Err(e) = io_result {
            error!("ACP Agent I/O 错误: {}", e);
        }

        info!("WebSocket 连接处理完成");
        Ok(())
    }
}

#[async_trait(?Send)]
impl AcpConnection for WssConnection {
    async fn run(&self, config: Config) -> Result<()> {
        let port = self.config.wss_port.unwrap_or(8338);
        let addr = format!("0.0.0.0:{}", port);

        info!("启动 ACP Agent (wss 模式) 监听地址: {}", addr);

        // 创建 TCP 监听器
        let listener = match TcpListener::bind(&addr).await {
            Ok(listener) => listener,
            Err(e) => {
                error!("无法绑定地址 {}: {}", addr, e);
                return Err(anyhow::anyhow!("无法绑定地址 {}: {}", addr, e));
            }
        };

        info!("WebSocket 服务器已启动，监听地址: {}", addr);

        // 使用 LocalSet 来运行非 Send 的 future
        let local_set = LocalSet::new();

        local_set
            .run_until(async move {
                loop {
                    // 接受新连接
                    match listener.accept().await {
                        Ok((stream, addr)) => {
                            info!("新的 TCP 连接来自: {}", addr);

                            // 为每个连接创建独立的配置副本
                            let stream_config = config.clone();
                            let connection_config = self.config.clone();
                            let server_name = connection_config.server_name.clone();
                            let server_version = connection_config.server_version.clone();

                            // 在新任务中处理连接
                            tokio::task::spawn_local(async move {
                                if let Err(e) = WssConnection::new(connection_config)
                                    .handle_connection(
                                        stream,
                                        stream_config,
                                        server_name,
                                        server_version,
                                    )
                                    .await
                                {
                                    error!("处理 WebSocket 连接失败: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            error!("接受连接失败: {}", e);
                        }
                    }
                }
            })
            .await;

        Ok(())
    }
}

/// 无界发送器写入器，实现 futures::AsyncWrite 接口
struct UnboundedSenderWriter {
    sender: mpsc::UnboundedSender<String>,
    buffer: Vec<u8>,
}

impl UnboundedSenderWriter {
    fn new(sender: mpsc::UnboundedSender<String>) -> Self {
        Self {
            sender,
            buffer: Vec::new(),
        }
    }
}

impl futures::AsyncWrite for UnboundedSenderWriter {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        self.buffer.extend_from_slice(buf);
        std::task::Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        if !self.buffer.is_empty() {
            let text = String::from_utf8_lossy(&self.buffer).to_string();
            if let Err(e) = self.sender.send(text) {
                return std::task::Poll::Ready(Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                )));
            }
            self.buffer.clear();
        }
        std::task::Poll::Ready(Ok(()))
    }

    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Ready(Ok(()))
    }
}

/// 无界接收器读取器，实现 futures::AsyncRead 接口
struct UnboundedReceiverReader {
    receiver: mpsc::UnboundedReceiver<String>,
    buffer: Vec<u8>,
    pos: usize,
}

impl UnboundedReceiverReader {
    fn new(receiver: mpsc::UnboundedReceiver<String>) -> Self {
        Self {
            receiver,
            buffer: Vec::new(),
            pos: 0,
        }
    }
}

impl futures::AsyncRead for UnboundedReceiverReader {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        // 如果缓冲区有数据，先返回缓冲区的内容
        if self.pos < self.buffer.len() {
            info!("poll read");
            let available = self.buffer.len() - self.pos;
            let len = std::cmp::min(buf.len(), available);
            buf[..len].copy_from_slice(&self.buffer[self.pos..self.pos + len]);
            self.pos += len;
            return std::task::Poll::Ready(Ok(len));
        }

        // 重置缓冲区位置
        self.buffer.clear();
        self.pos = 0;

        // 从接收器获取新数据
        info!("poll read");
        match self.receiver.poll_recv(cx) {
            std::task::Poll::Ready(Some(text)) => {
                info!("poll read {}", text);
                self.buffer.extend_from_slice(text.as_bytes());
                let len = std::cmp::min(buf.len(), self.buffer.len());
                buf[..len].copy_from_slice(&self.buffer[..len]);
                self.pos = len;
                std::task::Poll::Ready(Ok(len))
            }
            std::task::Poll::Ready(None) => {
                info!("poll read");
                std::task::Poll::Ready(Ok(0)) // EOF
            }
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

/// 创建连接
pub fn create_connection(config: ConnectionConfig) -> Box<dyn AcpConnection> {
    match config.connection_type {
        ConnectionType::Stdio => Box::new(StdioConnection::new(config)),
        ConnectionType::Wss => Box::new(WssConnection::new(config)),
    }
}

/// 运行 ACP Agent
pub async fn run_acp_agent(connection_config: ConnectionConfig) -> Result<()> {
    // 加载配置
    let config = Config::local().map_err(|e| anyhow::anyhow!("加载配置失败: {}", e))?;

    // 创建并运行连接
    let connection = create_connection(connection_config);
    connection.run(config).await
}
