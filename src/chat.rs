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
    /// 最大保存上下文数量
    max_context_num: usize,
}

impl Default for Chat {
    fn default() -> Self {
        Self::new(config::Config::local().unwrap(), CHAT_PROMPT.into())
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
    pub fn new(config: Config, system: String) -> Self {
        let max_try = max(config.max_tool_try, 0);
        Self {
            client: ChatClient::new(
                config.deepseek_key,
                config.url.unwrap_or("https://api.deepseek.com".into()),
                config.model.unwrap_or("deepseek-chat".into()),
                vec![],
            ),
            context: vec![ModelMessage::system(system)],
            max_tool_try: max_try,
            cancel_token: tokio_util::sync::CancellationToken::new(),
            running: false,
            max_context_num: max(config.max_context_num, 5),
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
        self.client.tools(tools);
        self
    }

    #[allow(unused)]
    pub fn max_try(mut self, max_try: usize)->Self {
        self.max_tool_try = max_try;
        self
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
                            Ok(res) => {
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
                            },
                            Err(e) => yield Ok(StreamedChatResponse::Text(e.to_string())),
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
        let cancel_token = self.cancel_token.clone();
        self.running = true;
        stream! {
            let mut count = self.max_tool_try;
            loop {
                let mut msg = ModelMessage::assistant("".into(), "".into(), vec![]);
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
                        Ok(res) => {
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
                        },
                        Err(e) => yield Ok(StreamedChatResponse::Text(e.to_string())),
                    }
                }
                yield Ok(StreamedChatResponse::End);
                self.context.push(msg.clone());
                if self.context.len() > self.max_context_num {
                    self.context.remove(0);
                }
                // 处理工具调用
                info!("工具数 {:?}", msg.tool_calls);
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
                        self.context.push(response);
                        if self.context.len() > self.max_context_num {
                            self.context.remove(0);
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

    fn call_tool(&self, tool_calls: Vec<ToolCall>)->impl Stream<Item = anyhow::Result<ModelMessage>> + '_ {
        let cancel_token = self.cancel_token.clone();
        stream! {
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
        if self.context.len() > self.max_context_num {
            self.context.remove(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self, Write};
    use futures::{StreamExt, pin_mut};
    use crate::{config, mcp, prompt::CHAT_PROMPT};

    use super::*;

    #[allow(unused)]
    async fn test_chat_streaming() -> Result<(), Box<dyn std::error::Error>> {
        log4rs::init_file("log4rs.yaml", Default::default()).unwrap();
        mcp::init().await;
        let mut chat = Chat::new(config::Config::local().unwrap(), CHAT_PROMPT.into())
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
        let mut chat = Chat::new(config::Config::local().unwrap(), CHAT_PROMPT.into())
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
