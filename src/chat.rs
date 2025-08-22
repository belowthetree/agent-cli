use futures::Stream;
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
        self.client
            .stream_chat(prompt, self.context.clone())
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
    use super::*;
    
}
