use futures::Stream;
use serde_json::Value;
use std::collections::HashMap;

use crate::client::chat_client::{ChatClient, ChatResult, StreamedChatResponse};
use crate::config::Config;
use crate::mcp::{McpManager, McpTool};
use crate::model::param::ModelMessage;

pub struct Chat {
    pub client: ChatClient,
    pub context: Vec<ModelMessage>,
    tool_map: HashMap<String, String>, // 工具名称 -> 服务器名称映射
}

impl Chat {
    pub fn new(config: Config) -> Self {
        let tools = McpManager::global().get_all_tools();
        let mut tool_map = HashMap::new();
        
        // 构建工具名称到服务器名称的映射
        for tool in &tools {
            tool_map.insert(tool.name(), tool.server_name().to_string());
        }
        
        Self {
            client: ChatClient::new(config.deepseek_key, tools),
            context: vec![],
            tool_map,
        }
    }

    pub async fn chat(&mut self, prompt: &str) -> anyhow::Result<ChatResult> {
        let resp = self.client.chat(prompt, self.context.clone()).await;
        
        // 执行工具调用
        if !resp.tool_calls.is_empty() {
            for tool_call in &resp.tool_calls {
                // 解析工具调用参数
                let arguments: Value = serde_json::from_str(&tool_call.function.arguments)
                    .unwrap_or_else(|_| Value::Object(serde_json::Map::new()));
                
                // 获取服务器名称
                let server_name = self.tool_map.get(&tool_call.function.name)
                    .map(|s| s.as_str())
                    .unwrap_or("");
                
                // 调用工具
                let tool_result = McpManager::global().call_tool(
                    server_name,
                    &tool_call.function.name,
                    &arguments
                ).await;
                println!("{:?}", tool_result);
                // 将工具调用结果添加到上下文
                if let Ok(result) = tool_result {
                    self.context.push(ModelMessage::tool(result, tool_call.clone()));
                } else if let Err(e) = tool_result {
                    self.context.push(ModelMessage::tool(
                        format!("工具调用失败: {}", e),
                        tool_call.clone()
                    ));
                }
            }
        }

        let t = resp.clone();
        self.context.push(ModelMessage::assistant(resp.content, resp.think, resp.tool_calls));
        Ok(t)
    }

    pub fn stream_chat(
        &mut self,
        prompt: &str,
    ) -> impl Stream<Item = Result<StreamedChatResponse, anyhow::Error>> + '_ {
        self.client
            .stream_chat(prompt, self.context.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
}
