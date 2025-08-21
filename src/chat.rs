use futures::Stream;
use rmcp::model::Tool;
use serde_json::Value;
use std::cmp::max;
use std::collections::HashMap;

use crate::client::chat_client::{ChatClient, ChatResult, StreamedChatResponse};
use crate::config::Config;
use crate::mcp::{McpManager};
use crate::model::param::ModelMessage;

pub struct Chat {
    pub client: ChatClient,
    pub context: Vec<ModelMessage>,
    tool_map: HashMap<String, String>, // 工具名称 -> 服务器名称映射
    max_tool_try: usize,
}

impl Chat {
    pub fn new(config: Config, system: String) -> Self {
        let tools = McpManager::global().get_all_tools();
        let mut tool_map = HashMap::new();
        
        // 构建工具名称到服务器名称的映射
        for tool in &tools {
            tool_map.insert(tool.name(), tool.server_name().to_string());
        }
        
        Self {
            client: ChatClient::new(config.deepseek_key, system, tools, max(config.max_tool_try, 3)),
            context: vec![],
            tool_map,
            max_tool_try: max(config.max_tool_try, 1),
        }
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
