use async_stream::stream;
use futures::{pin_mut, stream, Stream, StreamExt};
use log::info;
use rmcp::model::Tool;
use serde_json::Value;
use std::cmp::max;
use std::collections::HashMap;
use std::fmt::Display;

use crate::client::chat_client::{ChatClient, ChatResult};
use crate::client::tool_client;
use crate::config::Config;
use crate::mcp::{McpManager, McpTool};
use crate::model::param::{ModelMessage, ToolCall};

pub struct Chat {
    pub client: ChatClient,
    pub context: Vec<ModelMessage>,
    max_tool_try: usize,
    cancel_token: tokio_util::sync::CancellationToken,
}

#[derive(Debug)]
pub struct ToolCallInfo {
    pub name: String,
    pub content: String,
}

impl ToolCallInfo {
    pub fn tool(tool: ToolCall)->Self {
        Self {
            name: tool.function.name,
            content: tool.function.arguments.to_string(),
        }
    }
}

impl Display for ToolCallInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "调用：{}\n参数：{}\n", self.name, self.content)
    }
}

#[derive(Debug)]
pub enum StreamedChatResponse {
    Text(String),
    ToolCall(ToolCallInfo),
    Reasoning(String),
    ToolResponse(String),
}

impl Chat {
    pub fn new(config: Config, system: String) -> Self {
        let max_try = max(config.max_tool_try, 0);
        Self {
            client: ChatClient::new(config.deepseek_key, system, vec![], max_try),
            context: vec![],
            max_tool_try: max_try,
            cancel_token: tokio_util::sync::CancellationToken::new(),
        }
    }

    pub fn cancel(&self) {
        self.cancel_token.cancel();
    }

    pub fn tools(mut self, tools: Vec<McpTool>)->Self {
        self.client.tools(tools);
        self
    }

    pub fn max_try(mut self, max_try: usize)->Self {
        self.max_tool_try = max_try;
        self.client.max_try(self.max_tool_try);
        self
    }

    pub fn chat(&mut self, prompt: &str) -> impl Stream<Item = Result<StreamedChatResponse, anyhow::Error>> + '_ {
        self.context.push(ModelMessage::user(prompt.to_string()));
        let cancel_token = self.cancel_token.clone();
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
                                        yield Ok(StreamedChatResponse::ToolCall(ToolCallInfo::tool(tool)));
                                    }
                                }
                            },
                            Err(e) => yield Ok(StreamedChatResponse::Text(e.to_string())),
                        }
                    }
                }
                self.context.push(msg.clone());
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
                                    yield Ok(StreamedChatResponse::ToolResponse(res.content.clone()));
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
                else {
                    break;
                }
            }
        }
    }

    pub fn stream_chat(
        &mut self,
        prompt: &str,
    ) -> impl Stream<Item = Result<StreamedChatResponse, anyhow::Error>> + '_ {
        self.context.push(ModelMessage::user(prompt.to_string()));
        let cancel_token = self.cancel_token.clone();
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
                                    yield Ok(StreamedChatResponse::ToolCall(ToolCallInfo::tool(tool)));
                                }
                            }
                        },
                        Err(e) => yield Ok(StreamedChatResponse::Text(e.to_string())),
                    }
                }
                self.context.push(msg.clone());
                // 处理工具调用
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
                                    yield Ok(StreamedChatResponse::ToolResponse(res.content.clone()));
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
                else {
                    break;
                }
            }
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
}

#[cfg(test)]
mod tests {
    use std::io::{self, Write};
    use futures::{StreamExt, pin_mut};
    use log::info;
    use crate::{config, mcp, prompt::CHAT_PROMPT};

    use super::*;

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
                    StreamedChatResponse::ToolCall(tool_call) => print!("{}", tool_call),
                    StreamedChatResponse::Reasoning(think) => print!("{}", think),
                    StreamedChatResponse::ToolResponse(tool) => print!("{}", tool),
                }
                io::stdout().flush();
            }
        }
        println!("\n流式响应结束");
        Ok(())
    }

    #[tokio::test]
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
                    StreamedChatResponse::ToolCall(tool_call) => print!("{}", tool_call),
                    StreamedChatResponse::Reasoning(think) => print!("{}", think),
                    StreamedChatResponse::ToolResponse(tool) => print!("{}", tool),
                }
                io::stdout().flush();
            }
        }
        println!("\n非流式响应结束");
        Ok(())
    }
}
