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
    max_context_num: usize,
}

/// Chat 构建器，用于改进初始化和配置验证
pub struct ChatBuilder {
    config: Config,
    tools: Vec<McpTool>,
    max_tokens: Option<Option<u32>>,
    ask_before_tool_execution: Option<bool>,
    max_context_num: Option<usize>,
}

impl ChatBuilder {
    /// 从配置创建新的构建器
    pub fn from_config(config: Config) -> Self {
        Self {
            config,
            tools: Vec::new(),
            max_tokens: None,
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
        let max_tokens = self.max_tokens.unwrap_or(self.config.max_tokens);
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

        let state = ChatState::new(
            client,
            context,
            max_tokens,
            ask_before_tool_execution,
        );

        Ok(Chat { state, max_context_num })
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
                        break;
                    }
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
            let stream = self.stream_rechat();
            pin_mut!(stream);
            while let Some(res) = stream.next().await {
                yield res;
            }
        }
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
}
