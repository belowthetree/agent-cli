use std::cmp::max;

use async_stream::stream;
use futures::{Stream, StreamExt, pin_mut};
use log::info;

use crate::config::{self, Config};
use crate::mcp::McpTool;
use crate::model::param::{ModelMessage, ToolCall};
use crate::prompt;

mod chat_state;
mod chat_stream;
mod chat_tools;

pub use chat_state::ChatState;
pub use chat_stream::{StreamedChatResponse};

#[derive(Clone)]
pub struct Chat {
    state: ChatState,
}

/// Chat 构建器，用于改进初始化和配置验证
pub struct ChatBuilder {
    config: Config,
    tools: Vec<McpTool>,
    max_tool_try: Option<usize>,
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
            max_tool_try: None,
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
        let max_tool_try = self.max_tool_try.unwrap_or(max(self.config.max_tool_try, 0));
        let max_tokens = self.max_tokens.unwrap_or(self.config.max_tokens);
        let ask_before_tool_execution = self.ask_before_tool_execution.unwrap_or(self.config.ask_before_tool_execution);
        let max_context_num = self.max_context_num.unwrap_or(self.config.max_context_num);

        // 验证最大工具尝试次数
        if max_tool_try > 10 {
            return Err("最大工具尝试次数不能超过10".to_string());
        }

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
            max_tool_try,
            max_tokens,
            ask_before_tool_execution,
            max_context_num,
        );

        Ok(Chat { state })
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
        self.state.is_running()
    }

    pub fn lock(&mut self) {
        self.state.lock();
    }

    pub fn unlock(&mut self) {
        self.state.unlock();
    }

    pub fn cancel(&self) {
        self.state.cancel();
    }

    /// 获取取消令牌的副本，用于在流处理期间取消聊天
    pub fn get_cancel_token(&self) -> tokio_util::sync::CancellationToken {
        self.state.get_cancel_token()
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

    #[allow(unused)]
    pub fn max_try(mut self, max_try: usize) -> Self {
        // 注意：这个方法现在无效，因为 max_tool_try 在 ChatState 中是只读的
        // 如果需要修改，需要在 ChatState 中添加相应方法
        self
    }

    pub fn is_waiting_tool(&self) -> bool {
        self.state.is_waiting_tool()
    }

    /// 检查是否需要询问用户确认工具调用
    pub fn should_ask_for_tool_confirmation(&self) -> bool {
        self.state.should_ask_for_tool_confirmation()
    }

    /// 设置等待工具确认状态
    pub fn set_waiting_tool_confirmation(&mut self, waiting: bool) {
        self.state.set_waiting_tool_confirmation(waiting);
    }

    /// 检查是否正在等待工具确认
    pub fn is_waiting_tool_confirmation(&self) -> bool {
        self.state.is_waiting_tool_confirmation()
    }

    /// 确认工具调用
    pub fn confirm_tool_call(&mut self) {
        self.state.confirm_tool_call();
    }

    /// 拒绝工具调用
    pub fn reject_tool_call(&mut self) {
        self.state.reject_tool_call();
    }

    /// 设置工具确认结果
    pub fn set_tool_confirmation_result(&mut self, approved: bool) {
        self.state.set_tool_confirmation_result(approved);
    }

    // 用已有的上下文再次发送给模型，用于突然中断的情况
    pub fn stream_rechat(&mut self) -> impl Stream<Item = Result<StreamedChatResponse, anyhow::Error>> + '_ {
        // 刷新消息缓冲区，确保所有消息都在上下文中
        self.state.flush_message_buffer();
        
        let mut tools = Vec::new();
        if let Some(last) = self.state.context().last() {
            if let Some(calls) = &last.tool_calls {
                for tool in calls {
                    tools.push(tool.clone());
                }
            }
        }
        stream! {
            info!("stream_rechat 工具数 {:?}", tools);
            let mut tool_responses = Vec::new();
            {
                let stream = self.call_tool(tools);
                pin_mut!(stream);
                while let Some(res) = stream.next().await {
                    match res {
                        Ok(res) => {
                            yield Ok(StreamedChatResponse::ToolResponse(res.clone()));
                            tool_responses.push(res);
                        }
                        Err(e) => {
                            yield Err(e);
                        }
                    }
                    if self.state.get_cancel_token().is_cancelled() {
                        break;
                    }
                }
            }
            for response in tool_responses {
                self.state.add_message(response);
            }
            let stream = self.handle_stream_chat();
            pin_mut!(stream);
            while let Some(res) = stream.next().await {
                yield res;
            }
        }
    }

    pub fn chat(&mut self, prompt: &str) -> impl Stream<Item = Result<StreamedChatResponse, anyhow::Error>> + '_ {
        self.state.add_message(ModelMessage::user(prompt.to_string()));
        let cancel_token = self.state.get_cancel_token();
        self.state.lock();
        stream! {
            let mut count = self.state.max_tool_try();
            loop {
                // 刷新消息缓冲区，确保所有消息都在上下文中
                self.state.flush_message_buffer();
                
                let mut msg = ModelMessage::assistant("", "", vec![]);
                {
                    let stream = self.state.client().chat2(self.state.context().to_vec());
                    pin_mut!(stream);
                    while let Some(res) = stream.next().await {
                        // 检查是否已取消
                        if cancel_token.is_cancelled() {
                            break;
                        }
                        info!("{:?}", res);
                    match res {
                        Ok(mut res) => {
                            if res.content.len() > 0 {
                                msg.add_content(res.content.clone());
                                yield Ok(StreamedChatResponse::Text(res.content.to_string()));
                            }
                            if res.think.len() > 0 {
                                msg.add_think(res.think.clone());
                                yield Ok(StreamedChatResponse::Reasoning(res.think.to_string()));
                            }
                            if let Some(tools) = res.tool_calls {
                                for tool in tools {
                                    msg.add_tool(tool.clone());
                                    yield Ok(StreamedChatResponse::ToolCall(tool));
                                }
                            }
                            // 保存token使用情况
                            if let Some(usage) = &res.token_usage {
                                // 先检查token限制（借用）
                                if self.state.check_token_limit(Some(usage)) {
                                    // 超过限制，停止生成
                                    break;
                                }
                                // 然后移动值
                                msg.token_usage = res.token_usage.take();
                            }
                        },
                            Err(e) => yield Err(anyhow::anyhow!(e.to_string())),
                        }
                    }
                    yield Ok(StreamedChatResponse::End);
                }
                self.state.add_message(msg.clone());
                if msg.tool_calls.is_some() && count > 0 {
                    count -= 1;
                    let tool_calls = msg.tool_calls.unwrap();
                    let mut tool_responses = Vec::new();
                    {
                        let stream = self.call_tool(tool_calls);
                        pin_mut!(stream);
                while let Some(res) = stream.next().await {
                    match res {
                        Ok(res) => {
                            yield Ok(StreamedChatResponse::ToolResponse(res.clone()));
                            tool_responses.push(res);
                        }
                                Err(e) => {
                                    yield Err(e);
                                }
                            }
                        }
                    }
                    for response in tool_responses {
                        self.state.add_message(response);
                    }
                }
                else {
                    break;
                }
            }
            self.state.unlock();
        }
    }

    pub fn stream_chat(
        &mut self,
        prompt: &str,
    ) -> impl Stream<Item = Result<StreamedChatResponse, anyhow::Error>> + '_ {
        self.state.add_message(ModelMessage::user(prompt.to_string()));
        self.handle_stream_chat()
    }

    pub fn handle_stream_chat(&mut self)-> impl Stream<Item = Result<StreamedChatResponse, anyhow::Error>> + '_ {
        let cancel_token = self.state.get_cancel_token();
        self.state.lock();
        stream! {
            let mut count = self.state.max_tool_try();
            loop {
                // 刷新消息缓冲区，确保所有消息都在上下文中
                self.state.flush_message_buffer();
                
                let mut msg = ModelMessage::assistant("", "", vec![]);
                {
                    let stream = self.state.client().stream_chat(self.state.context().to_vec());
                    pin_mut!(stream);
                    // 接收模型输出
                    while let Some(res) = stream.next().await {
                        // 检查是否已取消
                        if cancel_token.is_cancelled() {
                            break;
                        }
                        info!("{:?}", res);
                        match res {
                            Ok(mut res) => {
                        if res.content.len() > 0 {
                            msg.add_content(res.content.clone());
                            yield Ok(StreamedChatResponse::Text(res.content.to_string()));
                        }
                        if res.think.len() > 0 {
                            msg.add_think(res.think.clone());
                            yield Ok(StreamedChatResponse::Reasoning(res.think.to_string()));
                        }
                                if let Some(tools) = res.tool_calls {
                                    for tool in tools {
                                        msg.add_tool(tool.clone());
                                        yield Ok(StreamedChatResponse::ToolCall(tool));
                                    }
                                }
                                // 保存token使用情况
                                if let Some(usage) = &res.token_usage {
                                    // 先检查token限制（借用）
                                    if self.state.check_token_limit(Some(usage)) {
                                        // 超过限制，停止生成
                                        break;
                                    }
                                    // 然后移动值
                                    msg.token_usage = res.token_usage.take();
                                }
                            },
                            Err(e) => yield Err(anyhow::anyhow!(e.to_string())),
                        }
                    }
                }
                yield Ok(StreamedChatResponse::End);
                self.state.add_message(msg.clone());
                // 处理工具调用
                info!("工具数 {:?}", msg.tool_calls);
                if msg.tool_calls.is_some() && count > 0 {
                    count -= 1;
                    let tool_calls = msg.tool_calls.unwrap();
                    
                    // 检查是否需要询问用户确认
                    if self.should_ask_for_tool_confirmation() {
                        // 设置等待确认状态
                        self.set_waiting_tool_confirmation(true);
                        // 不执行工具调用，等待用户确认
                        break;
                    } else {
                        // 不需要询问，直接执行工具调用
                        let mut tool_responses = Vec::new();
                        {
                            let stream = self.call_tool(tool_calls);
                            pin_mut!(stream);
                            while let Some(res) = stream.next().await {
                                match res {
                                    Ok(res) => {
                                        yield Ok(StreamedChatResponse::ToolResponse(res.clone()));
                                        tool_responses.push(res);
                                    }
                                    Err(e) => {
                                        yield Err(e);
                                    }
                                }
                            }
                        }
                        for response in tool_responses {
                            self.state.add_message(response);
                        }
                    }
                }
                else {
                    break;
                }
            }
            self.state.unlock();
        }
    }

    pub fn call_tool(&self, tool_calls: Vec<ToolCall>)->impl Stream<Item = anyhow::Result<ModelMessage>> + '_ {
        let cancel_token = self.state.get_cancel_token();
        stream! {
            if tool_calls.is_empty() {
                return;
            }
            let caller = crate::client::tool_client::ToolClient;
            let stream = caller.call(tool_calls);
            pin_mut!(stream);
            while let Some(res) = stream.next().await {
                if cancel_token.is_cancelled() {
                    return;
                }
                yield res;
            }
        }
    }

    pub fn add_message(&mut self, msg: ModelMessage) {
        self.state.add_message(msg);
    }

    /// 增加对话轮次计数
    pub fn increment_conversation_turn(&mut self) {
        self.state.increment_conversation_turn();
    }

    /// 重置对话轮次计数
    pub fn reset_conversation_turn(&mut self) {
        self.state.reset_conversation_turn();
    }

    /// 检查是否超过最大对话轮次
    pub fn is_over_context_limit(&self) -> bool {
        self.state.is_over_context_limit()
    }

    /// 设置等待对话轮次确认状态
    pub fn set_waiting_context_confirmation(&mut self, waiting: bool) {
        self.state.set_waiting_context_confirmation(waiting);
    }

    /// 检查是否正在等待对话轮次确认
    pub fn is_waiting_context_confirmation(&self) -> bool {
        self.state.is_waiting_context_confirmation()
    }

    /// 获取当前对话轮次统计
    pub fn get_conversation_turn_info(&self) -> (usize, usize) {
        self.state.get_conversation_turn_info()
    }

    /// 获取聊天上下文
    pub fn context(&self) -> &Vec<ModelMessage> {
        self.state.context()
    }

    /// 获取可变的聊天上下文
    pub fn context_mut(&mut self) -> &mut Vec<ModelMessage> {
        self.state.context_mut()
    }

    pub fn clear_context(&mut self) {
        self.state.context_mut().clear();
    }
}
