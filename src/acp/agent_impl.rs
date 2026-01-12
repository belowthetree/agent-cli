//! ACP Agent 实现，使用 agent-client-protocol 库
//! 
//! 实现 Agent trait，将 agent-cli 的功能暴露为 ACP 服务

use agent_client_protocol::{self as acp, Client, TextContent};
use async_trait::async_trait;
use futures::{pin_mut, StreamExt};
use log::{info, error, warn, debug};
use serde_json::json;
use tokio_util::sync::CancellationToken;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
use uuid::Uuid;

use crate::chat::Chat;
use crate::config::Config;
use crate::mcp::get_config_tools;

/// 会话更新发送器
type SessionUpdateSender = mpsc::UnboundedSender<(acp::SessionNotification, oneshot::Sender<()>)>;

/// 会话数据
#[derive(Clone)]
#[allow(dead_code)]
struct SessionData {
    id: acp::SessionId,
    cwd: PathBuf,
    chat: Chat,
}

/// ACP Agent 实现
pub struct AcpAgent {
    sessions: Arc<RwLock<HashMap<acp::SessionId, SessionData>>>,
    session_update_tx: SessionUpdateSender,
    config: Config,
    agent_info: acp::Implementation,
    cancels: Arc<RwLock<HashMap<acp::SessionId, CancellationToken>>>,
}

impl AcpAgent {
    /// 创建新的 ACP Agent
    pub fn new(
        server_name: String,
        server_version: String,
        config: Config,
        session_update_tx: SessionUpdateSender,
    ) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            session_update_tx,
            config,
            agent_info: acp::Implementation::new(server_name, server_version)
                .title(Some("Agent CLI".to_string())),
            cancels: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 创建新会话的 Chat 实例
    fn create_chat(&self) -> Chat {
        let mut chat = Chat::new(self.config.clone());
        chat = chat.tools(get_config_tools());
        chat
    }

    /// 生成会话 ID
    fn generate_session_id(&self) -> acp::SessionId {
        acp::SessionId::new(Uuid::new_v4().to_string())
    }

    /// 处理提示并发送更新
    async fn handle_prompt_internal(
        &self,
        session_id: acp::SessionId,
        content_blocks: Vec<acp::ContentBlock>,
    ) -> acp::Result<acp::PromptResponse> {
        // 提取文本内容
        let mut full_prompt = String::new();
        for block in &content_blocks {
            if let acp::ContentBlock::Text(text_content) = block {
                full_prompt.push_str(&text_content.text);
                full_prompt.push('\n');
            }
        }
        full_prompt = full_prompt.trim().to_string();
        
        if full_prompt.is_empty() {
            return Err(acp::Error::invalid_params());
        }

        info!("处理提示 - 会话: {:?}, 内容长度: {}", session_id, full_prompt.len());

        // 获取会话并处理流式响应
        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(&session_id)
            .ok_or_else(|| acp::Error::invalid_params())?;
        {
            self.cancels.write().await.insert(session.id.clone(), session.chat.get_cancel_token());
        }

        // 使用流式处理
        let stream = session.chat.stream_chat(&full_prompt);
        pin_mut!(stream);

        // 处理流式响应
        let mut current_text = String::new();

        while let Some(result) = stream.next().await {
            match result {
                Ok(response) => {
                    match response {
                        crate::chat::StreamedChatResponse::Text(text) => {
                            current_text.push_str(&text);
                            
                            // 发送流式文本更新
                            let _ = self.send_session_update(
                                session_id.clone(),
                                acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(
                                    acp::ContentBlock::Text(TextContent::new(text))
                                ),
                            )).await;
                        }
                        crate::chat::StreamedChatResponse::ToolCall(call) => {
                            debug!("工具调用: {:?}", call);
                            let _ = self.send_session_update(
                                session_id.clone(),
                                acp::SessionUpdate::ToolCall(acp::ToolCall::new(call.id, call.function.name),
                            )).await;
                        }
                        crate::chat::StreamedChatResponse::ToolResponse(msg) => {
                            debug!("工具执行结果: {:?}", msg);
                            let _ = self.send_session_update(
                                session_id.clone(),
                                acp::SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate::new(
                                    msg.tool_call_id.to_string(), 
                                    acp::ToolCallUpdateFields::new()
                                    .content(vec![acp::ToolCallContent::Content(acp::Content::new(acp::ContentBlock::Text(acp::TextContent::new(msg.content.to_string()))))])
                                    .title(msg.name)
                            ))).await;
                        }
                        crate::chat::StreamedChatResponse::Reasoning(text) => {
                            debug!("推理内容: {}", text);
                            
                            // 推理内容可以作为注释发送
                            let _ = self.send_session_update(
                                session_id.clone(),
                                acp::SessionUpdate::AgentThoughtChunk(acp::ContentChunk::new(
                                    acp::ContentBlock::Text(TextContent::new(text))
                                ),
                            )).await;
                        }
                        crate::chat::StreamedChatResponse::TokenUsage(usage) => {
                            info!("Token 使用: {:?}", usage);
                        }
                        crate::chat::StreamedChatResponse::End => {
                            info!("流处理完成");
                        }
                    }
                }
                Err(e) => {
                    error!("流处理错误: {}", e);
                    // 发送错误更新
                    let _ = self.send_session_update(
                        session_id.clone(),
                        acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(
                            acp::ContentBlock::Text(acp::TextContent::new(format!("错误: {}", e)))),
                    )).await;
                    return Err(acp::Error::internal_error());
                }
            }
        }

        // 返回响应
        Ok(acp::PromptResponse::new(acp::StopReason::EndTurn))
    }

    /// 发送会话更新
    async fn send_session_update(
        &self,
        session_id: acp::SessionId,
        update: acp::SessionUpdate,
    ) -> acp::Result<()> {
        let (tx, rx) = oneshot::channel();
        
        self.session_update_tx
            .send((acp::SessionNotification::new(session_id, update), tx))
            .map_err(|_| acp::Error::internal_error())?;
        
        rx.await.map_err(|_| acp::Error::internal_error())
    }
}

#[async_trait(?Send)]
impl acp::Agent for AcpAgent {
    async fn initialize(&self, _request: acp::InitializeRequest) -> acp::Result<acp::InitializeResponse> {
        info!("初始化 ACP 连接 {:?}", _request);

        Ok(acp::InitializeResponse::new(acp::ProtocolVersion::V1)
            .agent_info(self.agent_info.clone())
            .agent_capabilities(acp::AgentCapabilities::new()))
    }

    async fn authenticate(&self, request: acp::AuthenticateRequest) -> acp::Result<acp::AuthenticateResponse> {
        info!("收到认证请求: {:?}", request);
        // 简单实现：不进行认证，直接返回成功
        Ok(acp::AuthenticateResponse::new())
    }

    async fn new_session(&self, request: acp::NewSessionRequest) -> acp::Result<acp::NewSessionResponse> {
        let session_id = self.generate_session_id();
        let cwd = request.cwd.clone();
        
        info!("创建新会话 - ID: {:?}, 工作目录: {:?}", session_id, cwd);

        let chat = self.create_chat();
        let session_data = SessionData {
            id: session_id.clone(),
            cwd: cwd.clone(),
            chat,
        };

        self.sessions.write().await.insert(session_id.clone(), session_data);

        Ok(acp::NewSessionResponse::new(session_id).modes(None))
    }

    async fn load_session(&self, request: acp::LoadSessionRequest) -> acp::Result<acp::LoadSessionResponse> {
        info!("收到加载会话请求: {:?}", request);
        // 暂不支持从持久化存储加载会话
        warn!("load_session 暂不支持从持久化存储加载");
        Err(acp::Error::method_not_found())
    }

    async fn prompt(&self, request: acp::PromptRequest) -> acp::Result<acp::PromptResponse> {
        info!("处理提示 - 会话: {:?}", request.session_id);
        self.handle_prompt_internal(request.session_id, request.prompt).await
    }

    async fn cancel(&self, request: acp::CancelNotification) -> acp::Result<()> {
        info!("收到取消请求 - 会话: {:?}", request.session_id);
        if let Some(token) = self.cancels.read().await.get(&request.session_id) {
            token.cancel();
            Ok(())
        } else {
            warn!("取消失败，会话不存在");
            Err(acp::Error::internal_error())
        }
    }

    async fn set_session_mode(&self, request: acp::SetSessionModeRequest) -> acp::Result<acp::SetSessionModeResponse> {
        info!("收到设置会话模式请求 - 会话: {:?}, 模式: {:?}", request.session_id, request.mode_id);
        
        // TODO: 实现模式切换逻辑
        // let mut sessions = self.sessions.write().await;
        // if let Some(session) = sessions.get_mut(&request.session_id) {
        //     // 更新模式
        // }
        
        Ok(acp::SetSessionModeResponse::new())
    }

    // #[cfg(feature = "unstable_session_model")]
    // async fn set_session_model(&self, request: acp::SetSessionModelRequest) -> acp::Result<acp::SetSessionModelResponse> {
    //     info!("收到设置会话模型请求 - 会话: {:?}, 模型: {:?}", request.session_id, request.model);
    //     Ok(acp::SetSessionModelResponse)
    // }

    // #[cfg(feature = "unstable_session_config_options")]
    // async fn set_session_config_option(
    //     &self,
    //     request: acp::SetSessionConfigOptionRequest,
    // ) -> acp::Result<acp::SetSessionConfigOptionResponse> {
    //     info!("收到设置会话配置请求: {:?}", request);
    //     Ok(acp::SetSessionConfigOptionResponse::new(vec![]))
    // }

    async fn ext_method(&self, request: acp::ExtRequest) -> acp::Result<acp::ExtResponse> {
        info!(
            "收到扩展方法调用: method={}, params={:?}",
            request.method,
            request.params
        );
        
        // 处理一些常见的扩展方法
        match request.method.trim() {
            "get_status" => {
                let status = json!({
                    "version": self.agent_info.version,
                    "sessions": self.sessions.read().await.len(),
                });
                Ok(acp::ExtResponse::new(serde_json::value::to_raw_value(&status)?.into()))
            }
            "list_sessions" => {
                let sessions: Vec<String> = self.sessions
                    .read()
                    .await
                    .keys()
                    .map(|id| id.0.to_string())
                    .collect();
                Ok(acp::ExtResponse::new(serde_json::value::to_raw_value(&json!({"sessions": sessions}))?.into()))
            }
            _ => {
                warn!("未知的扩展方法: {}", request.method);
                Err(acp::Error::method_not_found())
            }
        }
    }

    async fn ext_notification(&self, request: acp::ExtNotification) -> acp::Result<()> {
        info!(
            "收到扩展通知: method={}, params={:?}",
            request.method,
            request.params
        );
        Ok(())
    }
}

/// 运行 ACP Agent (stdio 模式)
pub async fn run_stdio_agent(server_name: String, server_version: String) -> anyhow::Result<()> {
    // 在 LocalSet 外面创建 Config，避免 Send/Sync 约束问题
    let config = Config::local().map_err(|e| anyhow::anyhow!("加载配置失败: {}", e))?;
    
    // 创建会话更新通道
    let (session_update_tx, mut session_update_rx) = mpsc::unbounded_channel();
    
    let agent = AcpAgent::new(server_name, server_version, config, session_update_tx);

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    // 使用 LocalSet 来运行非 Send 的 future
    let local_set = tokio::task::LocalSet::new();
    
    local_set.run_until(async move {
        // 创建连接
        let (conn, handle_io) = acp::AgentSideConnection::new(
            agent,
            stdout.compat_write(),  // outgoing: 写入响应到 stdout
            stdin.compat(),         // incoming: 从 stdin 读取请求
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
    }).await?;

    Ok(())
}
