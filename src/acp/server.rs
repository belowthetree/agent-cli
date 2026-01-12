use crate::acp::handler::{DefaultAcpHandler, RequestDispatcher};
use crate::acp::types::{AcpError, AcpResult, JsonRpcRequest, JsonRpcNotification, JsonRpcResponse, JsonRpcError};
use crate::acp::transport::{Transport, TransportConfig, create_transport, TransportType};
use futures::StreamExt;
use std::sync::Arc;
use tokio::sync::RwLock;

/// ACP服务器
pub struct AcpServer {
    transport: Arc<Box<dyn Transport>>,
    dispatcher: Arc<RequestDispatcher>,
    is_running: Arc<RwLock<bool>>,
}

impl AcpServer {
    /// 创建新的ACP服务器
    pub fn new(
        server_name: String,
        server_version: String,
        config: TransportConfig,
    ) -> AcpResult<Self> {
        let transport = create_transport(config)?;
        
        let handler = DefaultAcpHandler::new(server_name, server_version);
        let dispatcher = RequestDispatcher::new(Box::new(handler));

        Ok(Self {
            transport: Arc::new(transport),
            dispatcher: Arc::new(dispatcher),
            is_running: Arc::new(RwLock::new(false)),
        })
    }

    /// 启动服务器
    pub async fn start(&self) -> AcpResult<()> {
        *self.is_running.write().await = true;
        
        log::info!("ACP服务器已启动");
        
        let transport = self.transport.clone();
        let dispatcher = self.dispatcher.clone();
        let is_running = self.is_running.clone();

        tokio::spawn(async move {
            let mut stream = transport.receive_stream();
            while *is_running.read().await {
                match stream.next().await {
                    Some(Ok(message)) => {
                        if let Err(e) = Self::process_message(&transport, &dispatcher, &message).await {
                            log::error!("处理消息失败: {}", e);
                            
                            // 发送错误响应
                            if let Ok(request) = serde_json::from_str::<JsonRpcRequest>(&message) {
                                let error_response = Self::create_error_response(request.id, e);
                                if let Err(e) = transport.send_response(&serde_json::to_string(&error_response).unwrap()).await {
                                    log::error!("发送错误响应失败: {}", e);
                                }
                            }
                        }
                    }
                    Some(Err(e)) => {
                        log::error!("接收消息错误: {}", e);
                    }
                    None => {
                        log::info!("消息流已结束");
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    /// 处理单条消息
    async fn process_message(
        transport: &Arc<Box<dyn Transport>>,
        dispatcher: &Arc<RequestDispatcher>,
        message: &str,
    ) -> AcpResult<()> {
        // 尝试解析为请求
        if let Ok(request) = serde_json::from_str::<JsonRpcRequest>(message) {
            let response = dispatcher.handle_request(request).await;
            let response_json = serde_json::to_string(&response)?;
            transport.send_response(&response_json).await?;
        }
        // 尝试解析为通知（无需响应）
        else if let Ok(_notification) = serde_json::from_str::<JsonRpcNotification>(message) {
            // 处理通知
            log::debug!("收到通知: {}", message);
        }
        else {
            return Err(AcpError::InvalidRequest);
        }

        Ok(())
    }

    /// 创建错误响应
    fn create_error_response(id: u64, error: AcpError) -> JsonRpcResponse {
        let (code, message) = match &error {
            AcpError::ParseError(_) => (-32700, "解析错误".to_string()),
            AcpError::InvalidRequest => (-32600, "无效请求".to_string()),
            AcpError::MethodNotFound(_) => (-32601, "方法不存在".to_string()),
            AcpError::InvalidParams(_) => (-32602, "无效参数".to_string()),
            _ => (-32603, "内部错误".to_string()),
        };

        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message,
                data: Some(serde_json::Value::String(format!("{}", error))),
            }),
        }
    }

    /// 发送通知
    pub async fn send_notification(&self, method: &str, params: serde_json::Value) -> AcpResult<()> {
        let notification = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
        };
        
        let notification_json = serde_json::to_string(&notification)?;
        self.transport.send_notification(&notification_json).await
    }

    /// 停止服务器
    pub async fn stop(&self) -> AcpResult<()> {
        *self.is_running.write().await = false;
        self.transport.close().await?;
        log::info!("ACP服务器已停止");
        Ok(())
    }

    /// 检查是否运行中
    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }
}

/// 便捷函数：创建stdio服务器
pub fn create_stdio_server(
    server_name: String,
    server_version: String,
) -> AcpResult<AcpServer> {
    let config = TransportConfig {
        transport_type: TransportType::Stdio,
        addr: None,
        port: None,
    };
    AcpServer::new(server_name, server_version, config)
}

/// 便捷函数：创建WebSocket服务器
pub fn create_ws_server(
    server_name: String,
    server_version: String,
    addr: String,
    port: u16,
) -> AcpResult<AcpServer> {
    let config = TransportConfig {
        transport_type: TransportType::WebSocket,
        addr: Some(addr),
        port: Some(port),
    };
    AcpServer::new(server_name, server_version, config)
}

/// 便捷函数：创建HTTP服务器
pub fn create_http_server(
    server_name: String,
    server_version: String,
    addr: String,
    port: u16,
) -> AcpResult<AcpServer> {
    let config = TransportConfig {
        transport_type: TransportType::Http,
        addr: Some(addr),
        port: Some(port),
    };
    AcpServer::new(server_name, server_version, config)
}
