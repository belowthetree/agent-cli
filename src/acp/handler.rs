use crate::acp::types::{
    AcpError, AcpResult, JsonRpcRequest, JsonRpcResponse, JsonRpcError, InitializeParams,
    SessionNewParams, SessionPromptParams, InitializeResult, SessionNewResult,
    SessionPromptResult, ServerInfo, Capabilities,
};
use async_trait::async_trait;
use serde_json::json;
use std::collections::HashMap;
use tokio::sync::RwLock;

/// 会话数据
#[derive(Debug, Clone)]
struct SessionData {
    id: String,
    cwd: String,
}

/// ACP处理器trait
#[async_trait]
pub trait AcpHandler: Send + Sync {
    /// 处理初始化
    async fn handle_initialize(&self, params: InitializeParams) -> AcpResult<InitializeResult>;
    
    /// 处理创建会话
    async fn handle_session_new(&self, params: SessionNewParams) -> AcpResult<SessionNewResult>;
    
    /// 处理发送提示
    async fn handle_session_prompt(&self, params: SessionPromptParams) -> AcpResult<SessionPromptResult>;
}

/// 默认ACP处理器实现
pub struct DefaultAcpHandler {
    sessions: RwLock<HashMap<String, SessionData>>,
    server_info: ServerInfo,
}

impl DefaultAcpHandler {
    pub fn new(server_name: String, server_version: String) -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            server_info: ServerInfo {
                name: server_name,
                version: server_version,
            },
        }
    }

    /// 生成会话ID
    fn generate_session_id(&self) -> String {
        format!("session-{}", uuid::Uuid::new_v4())
    }
}

#[async_trait]
impl AcpHandler for DefaultAcpHandler {
    async fn handle_initialize(&self, params: InitializeParams) -> AcpResult<InitializeResult> {
        log::info!("初始化连接 - 客户端: {} {}", params.client_info.name, params.client_info.version);
        
        // 验证协议版本
        if params.protocol_version != "0.1.0" {
            return Err(AcpError::InvalidParams("不支持的协议版本".to_string()));
        }

        Ok(InitializeResult {
            protocol_version: "0.1.0".to_string(),
            server_info: self.server_info.clone(),
            capabilities: Capabilities {
                tools: Some(true),
                resources: Some(true),
                streaming: Some(true),
            },
        })
    }

    async fn handle_session_new(&self, params: SessionNewParams) -> AcpResult<SessionNewResult> {
        let session_id = self.generate_session_id();
        
        log::info!("创建新会话 - ID: {}, 工作目录: {}", session_id, params.cwd);

        let session_data = SessionData {
            id: session_id.clone(),
            cwd: params.cwd,
        };

        self.sessions.write().await.insert(session_id.clone(), session_data);

        Ok(SessionNewResult { session_id })
    }

    async fn handle_session_prompt(&self, params: SessionPromptParams) -> AcpResult<SessionPromptResult> {
        let session_id = params.session_id.as_ref().ok_or_else(|| {
            AcpError::InvalidParams("缺少session_id".to_string())
        })?;

        // 验证会话是否存在
        let sessions = self.sessions.read().await;
        if !sessions.contains_key(session_id) {
            return Err(AcpError::SessionNotFound(session_id.clone()));
        }

        log::info!("处理提示 - 会话: {}, 内容: {}", session_id, params.prompt);

        // 这里应该触发实际的AI处理逻辑
        // 目前只是返回成功，实际实现需要集成到agent逻辑中
        
        Ok(SessionPromptResult { success: true })
    }
}

/// 请求调度器
pub struct RequestDispatcher {
    handler: Box<dyn AcpHandler>,
}

impl RequestDispatcher {
    pub fn new(handler: Box<dyn AcpHandler>) -> Self {
        Self { handler }
    }

    /// 处理JSON-RPC请求
    pub async fn handle_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        let id = request.id;
        let result = match request.method.as_str() {
            "initialize" => {
                match serde_json::from_value::<InitializeParams>(request.params.unwrap_or(json!({}))) {
                    Ok(params) => {
                        self.handler.handle_initialize(params).await
                            .map(|r| serde_json::to_value(r).unwrap())
                    }
                    Err(e) => Err(AcpError::InvalidParams(format!("参数解析错误: {}", e)))
                }
            }
            "session/new" => {
                match serde_json::from_value::<SessionNewParams>(request.params.unwrap_or(json!({}))) {
                    Ok(params) => {
                        self.handler.handle_session_new(params).await
                            .map(|r| serde_json::to_value(r).unwrap())
                    }
                    Err(e) => Err(AcpError::InvalidParams(format!("参数解析错误: {}", e)))
                }
            }
            "session/prompt" => {
                match serde_json::from_value::<SessionPromptParams>(request.params.unwrap_or(json!({}))) {
                    Ok(params) => {
                        self.handler.handle_session_prompt(params).await
                            .map(|r| serde_json::to_value(r).unwrap())
                    }
                    Err(e) => Err(AcpError::InvalidParams(format!("参数解析错误: {}", e)))
                }
            }
            _ => Err(AcpError::MethodNotFound(request.method)),
        };

        match result {
            Ok(result) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(result),
                error: None,
            },
            Err(e) => {
                let (code, message) = match &e {
                    AcpError::ParseError(_) => (-32700, "解析错误".to_string()),
                    AcpError::InvalidRequest => (-32600, "无效请求".to_string()),
                    AcpError::MethodNotFound(_) => (-32601, "方法不存在".to_string()),
                    AcpError::InvalidParams(_) => (-32602, "无效参数".to_string()),
                    AcpError::SessionNotFound(_) => (-32602, "会话不存在".to_string()),
                    _ => (-32603, "内部错误".to_string()),
                };

                JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: None,
                    error: Some(JsonRpcError {
                        code,
                        message,
                        data: Some(serde_json::Value::String(format!("{}", e))),
                    }),
                }
            }
        }
    }
}
