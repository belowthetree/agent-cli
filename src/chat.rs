use async_stream::stream;
use futures::{pin_mut, Stream, StreamExt};
use log::info;
use std::cmp::max;

use crate::client::chat_client::{ChatClient};
use crate::client::tool_client;
use crate::config::{self, Config};
use crate::mcp::{McpTool};
use crate::model::param::{ModelMessage, ToolCall};
use crate::prompt::CHAT_PROMPT;

#[derive(Clone)]
pub struct Chat {
    pub client: ChatClient,
    pub context: Vec<ModelMessage>,
    max_tool_try: usize,
    cancel_token: tokio_util::sync::CancellationToken,
    running: bool,
    /// token限制
    max_tokens: Option<u32>,
    /// 是否在工具执行前询问用户确认
    ask_before_tool_execution: bool,
    /// 是否正在等待用户确认工具调用
    waiting_tool_confirmation: bool,
    /// 对话轮次统计
    conversation_turn_count: usize,
    /// 最大对话轮次数
    max_context_num: usize,
    /// 是否正在等待对话轮次确认
    waiting_context_confirmation: bool,
}

impl Default for Chat {
    fn default() -> Self {
        Self::new(config::Config::local().unwrap())
    }
}

#[derive(Debug)]
pub enum StreamedChatResponse {
    Text(String),
    ToolCall(ToolCall),
    Reasoning(String),
    ToolResponse(ModelMessage),
    End,
}

impl Chat {
    pub fn new(config: Config) -> Self {
        let max_try = max(config.max_tool_try, 0);
        Self {
            client: ChatClient::new(
                config.api_key,
                config.url.unwrap_or("https://api.deepseek.com".into()),
                config.model.unwrap_or("deepseek-chat".into()),
                vec![],
            ),
            context: vec![ModelMessage::system(config.prompt.unwrap_or(CHAT_PROMPT.into()))],
            max_tool_try: max_try,
            cancel_token: tokio_util::sync::CancellationToken::new(),
            running: false,
            max_tokens: config.max_tokens,
            ask_before_tool_execution: config.ask_before_tool_execution,
            waiting_tool_confirmation: false,
            conversation_turn_count: 0,
            max_context_num: config.max_context_num,
            waiting_context_confirmation: false,
        }
    }

    pub fn is_running(&self)->bool {
        self.running
    }

    pub fn lock(&mut self) {
        self.running = true;
    }

    pub fn unlock(&mut self) {
        self.running = false;
    }

    pub fn cancel(&self) {
        self.cancel_token.cancel();
    }

    pub fn tools(mut self, tools: Vec<McpTool>)->Self {
        info!("设置工具 {}", tools.len());
        self.client.tools(tools);
        self
    }

    #[allow(unused)]
    pub fn max_try(mut self, max_try: usize)->Self {
        self.max_tool_try = max_try;
        self
    }

    pub fn is_waiting_tool(&self)->bool {
        if self.context.len() > 0 {
            if let Some(tools) = &self.context[self.context.len() - 1].tool_calls {
                return tools.len() > 0;
            }
        }
        false
    }

    /// 检查是否需要询问用户确认工具调用
    pub fn should_ask_for_tool_confirmation(&self) -> bool {
        self.ask_before_tool_execution
    }

    /// 设置等待工具确认状态
    pub fn set_waiting_tool_confirmation(&mut self, waiting: bool) {
        self.waiting_tool_confirmation = waiting;
    }

    /// 检查是否正在等待工具确认
    pub fn is_waiting_tool_confirmation(&self) -> bool {
        self.waiting_tool_confirmation
    }

    /// 确认工具调用
    pub fn confirm_tool_call(&mut self) {
        self.waiting_tool_confirmation = false;
    }

    /// 拒绝工具调用
    pub fn reject_tool_call(&mut self) {
        if let Some(last) = self.context.last().cloned() {
            if let Some(tools) = last.tool_calls {
                for tool in tools {
                    self.add_message(ModelMessage::tool("失败：用户拒绝".into(), tool));
                }
            }
        }
        self.waiting_tool_confirmation = false;
    }

    // 用已有的上下文再次发送给模型，用于突然中断的情况
    pub fn stream_rechat(&mut self) -> impl Stream<Item = Result<StreamedChatResponse, anyhow::Error>> + '_ {
        let mut tools= Vec::new();
        if let Some(last) = self.context.last() {
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
                    if self.cancel_token.is_cancelled() {
                        break;
                    }
                }
            }
            for response in tool_responses {
                self.context.push(response);
            }
            let stream = self.handle_stream_chat();
            pin_mut!(stream);
            while let Some(res) = stream.next().await {
                yield res;
            }
        }
    }

    pub fn chat(&mut self, prompt: &str) -> impl Stream<Item = Result<StreamedChatResponse, anyhow::Error>> + '_ {
        self.add_message(ModelMessage::user(prompt.to_string()));
        let cancel_token = self.cancel_token.clone();
        self.running = true;
        stream! {
            let mut count = self.max_tool_try;
            loop {
                let mut msg = ModelMessage::assistant("".into(), "".into(), vec![]);
                {
                    let stream = self.client.chat2(self.context.clone());
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
                                yield Ok(StreamedChatResponse::Text(res.content));
                            }
                            if res.think.len() > 0 {
                                msg.add_think(res.think.clone());
                                yield Ok(StreamedChatResponse::Reasoning(res.think));
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
                                if self.check_token_limit(Some(usage)) {
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
                self.add_message(msg.clone());
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
                        self.add_message(response);
                    }
                }
                else {
                    break;
                }
            }
            self.running = false;
        }
    }

    pub fn stream_chat(
        &mut self,
        prompt: &str,
    ) -> impl Stream<Item = Result<StreamedChatResponse, anyhow::Error>> + '_ {
        self.add_message(ModelMessage::user(prompt.to_string()));
        self.handle_stream_chat()
    }

    pub fn handle_stream_chat(&mut self)-> impl Stream<Item = Result<StreamedChatResponse, anyhow::Error>> + '_ {
        let cancel_token = self.cancel_token.clone();
        self.running = true;
        stream! {
            let mut count = self.max_tool_try;
            loop {
                let mut msg = ModelMessage::assistant("".into(), "".into(), vec![]);
                {
                    let stream = self.client.stream_chat(self.context.clone());
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
                                    yield Ok(StreamedChatResponse::Text(res.content));
                                }
                                if res.think.len() > 0 {
                                    msg.add_think(res.think.clone());
                                    yield Ok(StreamedChatResponse::Reasoning(res.think));
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
                                    if self.check_token_limit(Some(usage)) {
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
                self.context.push(msg.clone());
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
                            self.context.push(response);
                        }
                    }
                }
                else {
                    break;
                }
            }
            self.running = false;
        }
    }

    pub fn call_tool(&self, tool_calls: Vec<ToolCall>)->impl Stream<Item = anyhow::Result<ModelMessage>> + '_ {
        let cancel_token = self.cancel_token.clone();
        stream! {
            if tool_calls.is_empty() {
                return;
            }
            let caller = tool_client::ToolClient;
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

    fn add_message(&mut self, msg: ModelMessage) {
        self.context.push(msg);
    }

    /// 增加对话轮次计数
    pub fn increment_conversation_turn(&mut self) {
        self.conversation_turn_count += 1;
    }

    /// 重置对话轮次计数
    pub fn reset_conversation_turn(&mut self) {
        self.conversation_turn_count = 0;
    }

    /// 检查是否超过最大对话轮次
    pub fn is_over_context_limit(&self) -> bool {
        self.conversation_turn_count >= self.max_context_num
    }

    /// 设置等待对话轮次确认状态
    pub fn set_waiting_context_confirmation(&mut self, waiting: bool) {
        self.waiting_context_confirmation = waiting;
    }

    /// 检查是否正在等待对话轮次确认
    pub fn is_waiting_context_confirmation(&self) -> bool {
        self.waiting_context_confirmation
    }

    /// 获取当前对话轮次统计
    pub fn get_conversation_turn_info(&self) -> (usize, usize) {
        (self.conversation_turn_count, self.max_context_num)
    }

    /// 检查token使用是否超过限制
    /// 返回true表示超过限制，应该停止生成
    fn check_token_limit(&self, new_usage: Option<&crate::connection::TokenUsage>) -> bool {
        if let Some(max_tokens) = self.max_tokens {
            // 计算当前上下文的总token使用量
            let mut total_tokens = 0;
            
            // 累加所有消息的token使用量
            for message in &self.context {
                if let Some(usage) = &message.token_usage {
                    total_tokens += usage.total_tokens;
                }
            }
            
            // 如果提供了新的使用量，加上它
            if let Some(usage) = new_usage {
                total_tokens += usage.total_tokens;
            }
            
            // 检查是否超过限制
            if total_tokens > max_tokens {
                log::warn!("Token使用超过限制: {}/{} tokens", total_tokens, max_tokens);
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self, Write};
    use futures::{StreamExt, pin_mut};
    use crate::{config, mcp};

    use super::*;

    #[allow(unused)]
    async fn test_chat_streaming() -> Result<(), Box<dyn std::error::Error>> {
        log4rs::init_file("log4rs.yaml", Default::default()).unwrap();
        mcp::init().await;
        let mut chat = Chat::new(config::Config::local().unwrap())
        .tools(mcp::get_config_tools())
        .max_try(1);
        let stream = chat.stream_chat("请将“你好世界”写入到 E:\\Project\\temp\\test.txt 中");
        pin_mut!(stream);

        println!("开始接收流式响应:");
        while let Some(result) = stream.next().await {
            if let Ok(res) = result {
                match res {
                    StreamedChatResponse::Text(text) => print!("{}", text),
                    StreamedChatResponse::ToolCall(tool_call) => print!("{:?}", tool_call),
                    StreamedChatResponse::Reasoning(think) => print!("{}", think),
                    StreamedChatResponse::ToolResponse(tool) => print!("{:?}", tool),
                    StreamedChatResponse::End => {}
                }
                io::stdout().flush().unwrap();
            }
        }
        println!("\n流式响应结束");
        Ok(())
    }

    #[allow(unused)]
    async fn test_chat() -> Result<(), Box<dyn std::error::Error>> {
        log4rs::init_file("log4rs.yaml", Default::default()).unwrap();
        mcp::init().await;
        let mut chat = Chat::new(config::Config::local().unwrap())
        .tools(mcp::get_config_tools())
        .max_try(1);
        let stream = chat.chat("请将“你好世界”写入到 E:\\Project\\temp\\test.txt 中");
        pin_mut!(stream);

        println!("开始接收非流式响应:");
        while let Some(result) = stream.next().await {
            if let Ok(res) = result {
                match res {
                    StreamedChatResponse::Text(text) => print!("{}", text),
                    StreamedChatResponse::ToolCall(tool_call) => print!("{:?}", tool_call),
                    StreamedChatResponse::Reasoning(think) => print!("{}", think),
                    StreamedChatResponse::ToolResponse(tool) => print!("{:?}", tool),
                    StreamedChatResponse::End => {}
                }
                io::stdout().flush().unwrap();
            }
        }
        println!("\n非流式响应结束");
        Ok(())
    }
}
