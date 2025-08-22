use async_stream::stream;
use futures::{pin_mut, stream, Stream, StreamExt};
use log::info;
use rmcp::model::Tool;
use serde_json::Value;
use std::cmp::max;
use std::collections::HashMap;

use crate::client::chat_client::{ChatClient, ChatResult, StreamedChatResponse};
use crate::config::Config;
use crate::mcp::{McpManager, McpTool};
use crate::model::param::ModelMessage;

pub struct Chat {
    pub client: ChatClient,
    pub context: Vec<ModelMessage>,
    max_tool_try: usize,
}

impl Chat {
    pub fn new(config: Config, system: String) -> Self {
        let max_try = max(config.max_tool_try, 0);
        Self {
            client: ChatClient::new(config.deepseek_key, system, vec![], max_try),
            context: vec![],
            max_tool_try: max_try,
        }
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

    pub async fn chat(&mut self, prompt: &str) -> anyhow::Result<Vec<ChatResult>> {
        let mut resp = self.client.chat(prompt, self.context.clone()).await?;
        let mut res = Vec::new();
        for msg in resp.iter() {
            res.push(ChatResult{
                content: msg.content.clone(),
                tool_calls: msg.tool_calls.clone().unwrap_or(vec![]),
                think: msg.think.clone(),
            })
        }
        self.context.append(&mut resp);
        Ok(res)
    }

    pub fn stream_chat(
        &mut self,
        prompt: &str,
    ) -> impl Stream<Item = Result<StreamedChatResponse, anyhow::Error>> + '_ {
        self.context.push(ModelMessage::user(prompt.to_string()));
        stream! {
            let stream = self.client.stream_chat(self.context.clone());
            pin_mut!(stream);
            while let Some(res) = stream.next().await {
                info!("{:?}", res);
                match res {
                    Ok(res) => {
                        if res.content.len() > 0 {
                            yield Ok(StreamedChatResponse::Text(res.content.clone()));
                        }
                        if res.think.len() > 0 {
                            yield Ok(StreamedChatResponse::Reasoning(res.think.clone()));
                        }
                        if let Some(tools) = res.tool_calls {
                            for tool in tools {
                                yield Ok(StreamedChatResponse::ToolCall(tool));
                            }
                        }
                    },
                    Err(e) => yield Ok(StreamedChatResponse::Text(e.to_string())),
                }
            }
        }
    }

    pub async fn chat_with_tools(&mut self, prompt: &str, tools: &Vec<Tool>)->anyhow::Result<Vec<ChatResult>> {
        let mut resp = self.client.chat_with_tools(prompt, vec![], tools).await?;
        let mut res = Vec::new();
        for msg in resp.iter() {
            res.push(ChatResult{
                content: msg.content.clone(),
                tool_calls: msg.tool_calls.clone().unwrap_or(vec![]),
                think: msg.think.clone(),
            })
        }
        self.context.append(&mut resp);
        Ok(res)
    }


}

#[cfg(test)]
mod tests {
    use std::io::{self, Write};
    use futures::{StreamExt, pin_mut};
    use log::info;
    use crate::{config, mcp, prompt::CHAT_PROMPT};

    use super::*;

    #[tokio::test]
    async fn test_chat_streaming() -> Result<(), Box<dyn std::error::Error>> {
        log4rs::init_file("log4rs.yaml", Default::default()).unwrap();
        mcp::init().await;
        info!("开始");
        let mut chat = Chat::new(config::Config::local().unwrap(), CHAT_PROMPT.into());
        let stream = chat.stream_chat("请背诵长恨歌");
        pin_mut!(stream);

        println!("开始接收流式响应:");
        info!("开始2");
        while let Some(result) = stream.next().await {
            if let Ok(res) = result {
                match res {
                    StreamedChatResponse::Text(text) => print!("{}", text),
                    StreamedChatResponse::ToolCall(tool_call) => print!("{:?}", tool_call),
                    StreamedChatResponse::Reasoning(think) => print!("{}", think),
                }
                io::stdout().flush();
            }
        }
        info!("结束");
        println!("\n流式响应结束");
        Ok(())
    }
}