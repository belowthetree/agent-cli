//! ACP Agent 实现，使用 agent-client-protocol 库
//! 
//! 实现 Agent trait，将 agent-cli 的功能暴露为 ACP 服务

use agent_client_protocol::{self as acp};
use async_trait::async_trait;
use futures::{pin_mut, StreamExt};
use log::{info, error, warn};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
use uuid::Uuid;

use crate::chat::Chat;
use crate::chat::chat_stream::ChatStream;
use crate::config::Config;
use crate::mcp::get_config_tools;
use crate::model::param::ModelMessage;

/// 会话数据
#[derive(Clone)]
#[allow(dead_code)]
struct SessionData {
    id: acp::SessionId,
    cwd: PathBuf,
    chat: Chat,
    current_prompt: Arc<RwLock<Option<String>>>,
}

/// ACP Agent 实现
#[allow(dead_code)]
pub struct AcpAgent {
    sessions: Arc<RwLock<HashMap<acp::SessionId, SessionData>>>,
    config: Config,
    agent_info: acp::Implementation,
}

impl AcpAgent {
    /// 创建新的 ACP Agent
    pub fn new(server_name: String, server_version: String, config: Config) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            config,
            agent_info: acp::Implementation::new(
                server_name,
                server_version,
            ),
        }
    }

    /// 创建新会话的 Chat 实例
    fn create_chat(&self) -> Chat {
        let mut chat = Chat::new(self.config.clone());
        chat = chat.tools(get_config_tools());
        chat.run();
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

        // 发送用户消息更新
        let _ = self.send_user_message_update(&session_id, &full_prompt).await;

        // 获取会话并添加用户消息
        let sessions = self.sessions.read().await;
        let session_id_ref = &session_id;
        
        // 保存当前提示
        {
            let session = sessions.get(session_id_ref)
                .ok_or_else(|| acp::Error::invalid_params())?;
            *session.current_prompt.write().await = Some(full_prompt.clone());
        }

        // 创建一个新的写入锁来修改会话
        drop(sessions);
        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(session_id_ref)
            .ok_or_else(|| acp::Error::invalid_params())?;

        // 添加用户消息到聊天上下文
        session.chat.add_message(ModelMessage::user(full_prompt.clone()));

        // 使用流式处理
        let stream = ChatStream::handle_rechat(&mut session.chat);
        pin_mut!(stream);

        // 处理流式响应
        let mut current_text = String::new();

        while let Some(result) = stream.next().await {
            match result {
                Ok(response) => {
                    match response {
                        crate::chat::StreamedChatResponse::Text(text) => {
                            current_text.push_str(&text);
                            
                            // 发送流式更新
                            let _ = self.send_message_chunk_update(
                                session_id_ref,
                                &text,
                            ).await;
                        }
                        crate::chat::StreamedChatResponse::ToolCall(call) => {
                            // 发送工具调用更新
                            let _ = self.send_tool_call_update(
                                session_id_ref,
                                &call,
                            ).await;
                        }
                        crate::chat::StreamedChatResponse::ToolResponse(msg) => {
                            info!("工具执行结果: {:?}", msg);
                        }
                        crate::chat::StreamedChatResponse::Reasoning(text) => {
                            info!("推理内容: {}", text);
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
                    return Err(acp::Error::internal_error());
                }
            }
        }

        // 返回响应
        Ok(acp::PromptResponse::new(acp::StopReason::EndTurn))
    }

    /// 发送消息块更新
    async fn send_message_chunk_update(
        &self,
        _session_id: &acp::SessionId,
        content: &str,
    ) -> acp::Result<()> {
        // 注意：这里需要有一个方法来发送更新到客户端
        // 在实际实现中，需要在 Agent trait 中添加一个方法来发送通知
        // 目前这个方法只是占位符
        info!("发送消息块更新: {} 字符", content.len());
        Ok(())
    }

    /// 发送用户消息更新
    async fn send_user_message_update(
        &self,
        _session_id: &acp::SessionId,
        content: &str,
    ) -> acp::Result<()> {
        info!("发送用户消息更新: {}", content);
        Ok(())
    }

    /// 发送工具调用更新
    async fn send_tool_call_update(
        &self,
        _session_id: &acp::SessionId,
        call: &crate::model::param::ToolCall,
    ) -> acp::Result<()> {
        info!("发送工具调用更新: {:?}", call);
        Ok(())
    }
}

#[async_trait(?Send)]
impl acp::Agent for AcpAgent {
    async fn initialize(&self, _request: acp::InitializeRequest) -> acp::Result<acp::InitializeResponse> {
        info!("初始化 ACP 连接");

        Ok(acp::InitializeResponse::new(acp::ProtocolVersion::V1))
    }

    async fn new_session(&self, request: acp::NewSessionRequest) -> acp::Result<acp::NewSessionResponse> {
        let session_id = self.generate_session_id();
        let cwd = request.cwd;
        
        info!("创建新会话 - ID: {:?}, 工作目录: {:?}", session_id, cwd);

        let chat = self.create_chat();
        let session_data = SessionData {
            id: session_id.clone(),
            cwd: cwd.clone(),
            chat,
            current_prompt: Arc::new(RwLock::new(None)),
        };

        self.sessions.write().await.insert(session_id.clone(), session_data);

        Ok(acp::NewSessionResponse::new(session_id))
    }

    async fn prompt(&self, request: acp::PromptRequest) -> acp::Result<acp::PromptResponse> {
        info!("处理提示 - 会话: {:?}", request.session_id);

        self.handle_prompt_internal(request.session_id, request.prompt).await
    }

    async fn load_session(&self, _request: acp::LoadSessionRequest) -> acp::Result<acp::LoadSessionResponse> {
        warn!("load_session 暂不支持");
        Err(acp::Error::method_not_found())
    }

    async fn authenticate(&self, _request: acp::AuthenticateRequest) -> acp::Result<acp::AuthenticateResponse> {
        warn!("authenticate 暂不支持");
        Err(acp::Error::method_not_found())
    }

    async fn cancel(&self, _request: acp::CancelNotification) -> acp::Result<()> {
        warn!("cancel 暂不支持");
        Err(acp::Error::method_not_found())
    }
}

/// 运行 ACP Agent (stdio 模式)
pub async fn run_stdio_agent(server_name: String, server_version: String) -> anyhow::Result<()> {
    // 在 LocalSet 外面创建 Config，避免 Send/Sync 约束问题
    let config = Config::local().map_err(|e| anyhow::anyhow!("加载配置失败: {}", e))?;
    let agent = AcpAgent::new(server_name, server_version, config);

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    // 使用 LocalSet 来运行非 Send 的 future
    let local_set = tokio::task::LocalSet::new();
    
    local_set.run_until(async move {
        let (_conn, handle_io) = acp::AgentSideConnection::new(
            agent,
            stdout.compat_write(),  // outgoing: 写入响应到 stdout
            stdin.compat(),         // incoming: 从 stdin 读取请求
            |fut| {
                tokio::task::spawn_local(fut);
            },
        );

        // 处理 I/O 在后台
        tokio::task::spawn_local(handle_io);

        // 等待连接关闭 - 只使用 Ctrl+C
        tokio::signal::ctrl_c().await?;
        info!("收到 Ctrl+C 信号，退出 ACP Agent");
        
        Ok::<(), anyhow::Error>(())
    }).await?;

    Ok(())
}
