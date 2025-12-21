use futures::{Stream, StreamExt, pin_mut};
use log::{info, warn};

use crate::config::{self, Config};
use crate::mcp::McpTool;
use crate::model::param::{ModelMessage};
use crate::prompt;

mod chat_state;
mod chat_stream;
mod chat_tools;

pub use chat_state::EChatState;
pub use chat_state::ChatState;
pub use chat_stream::{StreamedChatResponse};

#[derive(Clone)]
pub struct Chat {
    state: ChatState,
    max_context_num: usize,   // 保存token限制的副本
    auto_compress_threshold: f32,  // 自动压缩阈值（token使用比例）
}

/// Chat 构建器，用于改进初始化和配置验证
pub struct ChatBuilder {
    config: Config,
    tools: Vec<McpTool>,
    ask_before_tool_execution: Option<bool>,
    max_context_num: Option<usize>,
}

impl ChatBuilder {
    /// 从配置创建新的构建器
    pub fn from_config(config: Config) -> Self {
        Self {
            config,
            tools: Vec::new(),
            ask_before_tool_execution: None,
            max_context_num: None,
        }
    }

    /// 构建 Chat 实例，进行配置验证
    pub fn build(self) -> Result<Chat, String> {
        // 验证配置
        if self.config.api_key.is_empty() {
            return Err("API密钥不能为空".to_string());
        }

        // 使用构建器中的值或回退到配置中的默认值
        let ask_before_tool_execution = self.ask_before_tool_execution.unwrap_or(self.config.ask_before_tool_execution);
        let max_context_num = self.max_context_num.unwrap_or(self.config.max_context_num);

        // 验证最大对话轮次数
        if max_context_num == 0 {
            return Err("最大对话轮次数必须大于0".to_string());
        }

        let client = crate::client::chat_client::ChatClient::new(
            self.config.api_key.clone(),
            self.config.url.clone().unwrap_or("https://api.deepseek.com".into()),
            self.config.model.clone().unwrap_or("deepseek-chat".into()),
            self.tools,
        );

        let context = vec![ModelMessage::system(
            self.config.prompt.map(|p| prompt::build_enhanced_prompt(&p))
                .unwrap_or_else(|| prompt::get_default_enhanced_prompt())
        )];

        let tokens = client.get_token_limit();
        let state = ChatState::new(
            client,
            context,
            tokens,
            ask_before_tool_execution,
        );

        Ok(Chat { 
            state, 
            max_context_num,
            auto_compress_threshold: self.config.auto_compress_threshold,
        })
    }
}

impl Default for Chat {
    fn default() -> Self {
        Self::new(config::Config::local().unwrap())
    }
}

impl Chat {
    /// 使用配置创建新的 Chat 实例
    pub fn new(config: Config) -> Self {
        ChatBuilder::from_config(config)
            .build()
            .unwrap_or_else(|err| {
                log::error!("Chat 初始化失败: {}", err);
                panic!("Chat 初始化失败: {}", err);
            })
    }

    pub fn get_token_limit(&self)->u32 {
        self.state.client.get_token_limit()
    }

    pub fn is_running(&self) -> bool {
        self.state.get_state() == EChatState::Running
    }

    pub fn run(&mut self) {
        self.state.set_state(EChatState::Running);
    }

    /// 获取取消令牌的副本，用于在流处理期间取消聊天
    pub fn get_cancel_token(&self) -> tokio_util::sync::CancellationToken {
        self.state.get_cancel_token()
    }

    pub fn get_state(&self)->EChatState {
        self.state.get_state()
    }

    pub fn confirm(&mut self) {
        if self.get_state() == EChatState::WaitingToolConfirm {
            self.state.set_state(EChatState::WaitingToolUse);
        } else {
            self.state.set_state(EChatState::Idle);
        }
    }

    pub fn tools(mut self, tools: Vec<McpTool>) -> Self {
        info!("设置工具 {}", tools.len());
        self.state.set_tools(tools);
        self
    }

    pub fn set_tools(&mut self, tools: Vec<McpTool>) {
        info!("设置工具 {}", tools.len());
        self.state.set_tools(tools);
    }

    // 有工具调用没处理
    pub fn is_remain_tool_call(&self) -> bool {
        self.state.is_remain_tool_call()
    }

    // 是否正在等待工具调用确认
    pub fn is_need_tool_confirm(&self)-> bool {
        self.get_state() != EChatState::WaitingToolUse && self.state.should_tool_confirmation() && self.is_remain_tool_call()
    }

    /// 拒绝工具调用
    pub fn reject_tool_call(&mut self) {
        self.state.set_state(EChatState::Idle);
        for call in self.state.get_tool_calls() {
            self.add_message(ModelMessage::tool("用户拒绝调用", call));
        }
    }

    // 用已有的上下文再次发送给模型，用于突然中断的情况
    pub fn stream_rechat(&mut self) -> impl Stream<Item = Result<StreamedChatResponse, anyhow::Error>> + '_ {
        async_stream::stream! {
            loop {
                // 先判断是否超过轮次
                info!("对话轮次 {} {}", self.state.get_conversation_turn_info(), self.max_context_num);
                if self.is_over_context_limit() {
                    info!("超过对话轮次 {} {}", self.state.get_conversation_turn_info(), self.max_context_num);
                    self.state.set_state(EChatState::WaitingTurnConfirm);
                }
                if self.get_state() == EChatState::Idle || self.get_state() == EChatState::WaitingToolUse {
                    // 如果有工具调用需要确认，退出等待确认
                    if self.is_need_tool_confirm() {
                        warn!("等待工具确认");
                        self.state.set_state(EChatState::WaitingToolConfirm);
                        return;
                    }
                    self.state.set_state(EChatState::Running);
                    // 处理工具调用
                    {
                        let stream = chat_tools::ChatTools::handle_stream_tool(self, self.get_cancel_token());
                        pin_mut!(stream);
                        while let Some(res) = stream.next().await {
                            yield res;
                        }
                    }
                    // 处理聊天
                    {
                        // 对话轮数 + 1
                        self.state.increment_conversation_turn();
                        let stream = chat_stream::ChatStream::handle_rechat(self);
                        pin_mut!(stream);
                        while let Some(res) = stream.next().await {
                            yield res;
                        }
                    }
                    // 聊天结束可能产生新的工具调用
                    if self.is_need_tool_confirm() {
                        warn!("等待工具确认");
                        self.state.set_state(EChatState::WaitingToolConfirm);
                        break;
                    }
                    // 无工具调用，退出循环
                    if !self.is_remain_tool_call() {
                        info!("对话结束");
                        self.state.set_state(EChatState::Idle);
                        break;
                    }
                    info!("有工具需要调用");
                } else {
                    warn!("正在运行");
                    yield Err(anyhow::anyhow!("对话不在空闲状态，当前状态：{:?}", self.get_state()));
                    break;
                }
            }
        }
    }

    pub fn chat<'a, 'b>(&'a mut self, prompt: &'b str) -> impl Stream<Item = Result<StreamedChatResponse, anyhow::Error>> + 'a 
    where
        'b: 'a,
    {
        async_stream::stream! {
            loop {
                // 先判断是否超过轮次
                if self.is_over_context_limit() {
                    info!("超过对话轮次 {} {}", self.state.get_conversation_turn_info(), self.max_context_num);
                    self.state.set_state(EChatState::WaitingTurnConfirm);
                }
                if self.get_state() == EChatState::Idle || self.get_state() == EChatState::WaitingToolUse {
                    // 如果有工具调用需要确认，退出等待确认
                    if self.is_need_tool_confirm() {
                        warn!("等待工具确认");
                        self.state.set_state(EChatState::WaitingToolConfirm);
                        return;
                    }
                    self.state.set_state(EChatState::Running);
                    // 处理工具调用
                    {
                        let stream = chat_stream::ChatStream::handle_chat(&mut self.state, prompt);
                        pin_mut!(stream);
                        while let Some(res) = stream.next().await {
                            yield res;
                        }
                    }
                    // 处理聊天
                    {
                        // 对话轮数 + 1
                        self.state.increment_conversation_turn();
                        let stream = chat_stream::ChatStream::handle_rechat(self);
                        pin_mut!(stream);
                        while let Some(res) = stream.next().await {
                            yield res;
                        }
                    }
                    // 聊天结束可能产生新的工具调用
                    if self.is_need_tool_confirm() {
                        warn!("等待工具确认");
                        self.state.set_state(EChatState::WaitingToolConfirm);
                        break;
                    }
                    // 无工具调用，退出循环
                    if !self.is_remain_tool_call() {
                        info!("对话结束");
                        self.state.set_state(EChatState::Idle);
                        break;
                    }
                } else {
                    warn!("正在运行");
                    if self.is_over_context_limit() {
                        yield Err(anyhow::anyhow!("对话超过次数限制：{}", self.get_conversation_turn_info().1));
                    }
                    yield Err(anyhow::anyhow!("对话不在空闲状态，当前状态：{:?}", self.get_state()));
                    break;
                }
            }
        }
    }

    pub fn stream_chat<'a>(
        &'a mut self,
        prompt: &'a str,
    ) -> impl Stream<Item = Result<StreamedChatResponse, anyhow::Error>> + 'a {
        self.state.add_message(ModelMessage::user(prompt.to_string()));
        async_stream::stream! {
            // 检查是否需要自动压缩
            if self.should_auto_compress() {
                info!("检测到需要自动压缩，正在执行...");
                // 执行自动压缩
                let compressed = self.auto_compress_if_needed().await;
                if compressed {
                    info!("自动压缩完成，继续处理聊天");
                } else {
                    warn!("自动压缩失败，继续处理聊天");
                }
            }
            
            let stream = self.stream_rechat();
            pin_mut!(stream);
            while let Some(res) = stream.next().await {
                yield res;
            }
        }
    }

    /// 检查是否需要自动压缩
    pub fn should_auto_compress(&self) -> bool {
        // 获取max_tokens的值
        let max_tokens = self.get_token_limit();
        self.state.should_auto_compress(self.auto_compress_threshold, max_tokens)
    }

    /// 自动压缩对话（异步版本，供外部调用）
    pub async fn auto_compress_if_needed(&mut self) -> bool {
        if self.should_auto_compress() {
            info!("Token使用超过阈值，触发自动压缩");
            return self.compress_conversation().await;
        }
        false
    }

    pub fn add_message(&mut self, msg: ModelMessage) {
        self.state.add_message(msg);
    }

    /// 重置对话轮次计数
    pub fn reset_conversation_turn(&mut self) {
        self.state.reset_conversation_turn();
    }

    /// 检查是否超过最大对话轮次
    pub fn is_over_context_limit(&self) -> bool {
        self.state.get_conversation_turn_info() >= self.max_context_num
    }

    /// 获取当前对话轮次统计
    pub fn get_conversation_turn_info(&self) -> (usize, usize) {
        (self.state.get_conversation_turn_info(), self.max_context_num)
    }

    /// 获取聊天上下文
    pub fn context(&self) -> &Vec<ModelMessage> {
        self.state.context()
    }

    pub fn clear_context(&mut self) {
        self.state.context_mut().clear();
    }

    /// 主动向模型要求压缩对话，压缩后的对话将取代原对话上下文
    /// 返回压缩是否成功的布尔值
    pub async fn compress_conversation(&mut self) -> bool {
        // 临时保存当前上下文，以便压缩失败时恢复
        let original_context = self.state.context().clone();
        
        // 如果上下文为空或只有系统消息，不需要压缩
        if original_context.len() <= 1 {
            info!("上下文过短，无需压缩");
            return true;
        }

        // 构建压缩prompt
        let compress_prompt = self.build_compress_prompt();
        
        info!("开始压缩对话，当前上下文长度: {}", original_context.len());
        
        // 创建一个临时的压缩消息，包含当前上下文
        let mut compress_messages = vec![ModelMessage::system("你是一个对话压缩助手。请将以下对话压缩成简洁的摘要，保留关键信息和上下文，但大幅减少token使用。只返回压缩后的摘要内容，不要添加额外解释。")];
        
        // 添加除了系统消息外的所有消息作为参考
        for msg in &original_context[1..] {
            let role = msg.role.as_ref();
            let content = msg.content.as_ref();
            let think = if !msg.think.is_empty() {
                format!(" [思考: {}]", msg.think.as_ref())
            } else {
                String::new()
            };
            compress_messages.push(ModelMessage::user(format!("{}: {}{}", role, content, think)));
        }
        
        compress_messages.push(ModelMessage::user(compress_prompt));

        // 先获取client的引用，避免后续借用冲突
        let client = self.state.client().clone();
        
        // 使用非流式方式请求压缩
        self.state.set_state(EChatState::Compressing);
        let stream = client.chat2(compress_messages);
        pin_mut!(stream);
        
        let mut compressed_content = String::new();
        while let Some(res) = stream.next().await {
            match res {
                Ok(msg) => {
                    compressed_content.push_str(&msg.content);
                }
                Err(e) => {
                    warn!("压缩失败: {}", e);
                    self.state.set_state(EChatState::Idle);
                    return false;
                }
            }
        }
        self.state.set_state(EChatState::Idle);

        if compressed_content.trim().is_empty() {
            warn!("压缩返回空内容");
            return false;
        }

        // 构建新的压缩后上下文
        let mut new_context = Vec::new();
        
        // 保留系统消息
        if let Some(system_msg) = original_context.first() {
            if system_msg.role == "system" {
                new_context.push(system_msg.clone());
            }
        }
        
        // 添加压缩后的摘要作为用户消息
        new_context.push(ModelMessage::user(format!("对话历史摘要: {}", compressed_content)));
        
        // 替换上下文
        *self.state.context_mut() = new_context;
        
        // 重置对话轮次计数，因为现在上下文被压缩了
        self.state.reset_conversation_turn();
        
        info!("对话压缩成功，新上下文长度: {}", self.state.context().len());
        true
    }

    /// 构建压缩prompt
    fn build_compress_prompt(&self) -> String {
        r#"请将上面的对话历史压缩成一个简洁的摘要。要求：
1. 保留对话的核心主题和关键信息
2. 保留用户的重要需求和问题
3. 保留重要的技术细节和决策
4. 大幅减少冗余的对话轮次
5. 保持逻辑连贯性
6. 只返回压缩后的摘要，不要添加任何解释或说明

压缩后的摘要应该足够简洁，但包含所有必要信息，以便继续对话时能理解上下文。"#
.to_string()
    }
}
