use async_stream::stream;
use futures::{Stream, StreamExt};
use rmcp::model::Tool;
use serde_json::Value;
use tokio::runtime::Runtime;

use crate::{connection::CommonConnectionContent, mcp::{McpManager, McpTool}, model::{deepseek, param::{ModelInputParam, ModelMessage, ToolCall}, AgentModel}};

pub struct ChatClient {
    pub agent: deepseek::DeepseekModel,
    system: String,
    tools: Vec<McpTool>,
    max_tool_loop: usize,
}

#[derive(Debug, Clone)]
pub struct ChatResult {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub think: String,
}

#[derive(Debug)]
pub enum StreamedChatResponse {
    Text(String),
    ToolCall(ToolCall),
    Reasoning(String),
}

impl ChatClient {
    pub fn new(key: String, system: String, tools: Vec<McpTool>, max_tool_loop: usize) -> Self {
        let agent = deepseek::DeepseekModel::new("https://api.deepseek.com/chat/completions".into(), "deepseek-chat".into(), key);
        Self {
            agent,
            system,
            tools,
            max_tool_loop,
        }
    }

    pub async fn chat(&mut self, prompt: &str, chat_history: Vec<ModelMessage>) -> anyhow::Result<Vec<ModelMessage>> {
        let mut tools = Vec::new();
        for tool in self.tools.iter() {
            tools.push(tool.clone().into());
        }
        self.chat_with_tools(prompt, chat_history, &tools).await
    }

    pub async fn chat_with_tools(&mut self, prompt: &str, mut chat_history: Vec<ModelMessage>, tools: &Vec<Tool>)->anyhow::Result<Vec<ModelMessage>> {
        if self.system.len() > 0 {
            chat_history.push(ModelMessage::system(self.system.to_string()));
        }
        if prompt.len() > 0 {
            chat_history.push(ModelMessage::user(prompt.to_string()));
        }
        let param = ModelInputParam{
            temperature: None,
            tools: Some(tools.clone()),
            messages: chat_history.clone(),
        };
        let (res, tool_calls) = self.chat_internal(param).await?;
        self.tool_call(tool_calls, self.max_tool_loop, &mut chat_history, tools).await?;
        Ok(res)
    }

    pub fn stream_chat(&self, prompt: &str, mut chat_history: Vec<ModelMessage>)-> impl Stream<Item = Result<StreamedChatResponse, anyhow::Error>> + '_ {
        let prompt = prompt.to_string();
        let agent = self.agent.clone();
        stream! {
            chat_history.push(ModelMessage::user(prompt.to_string()));
            let mut tools = Vec::new();
            for tool in self.tools.iter() {
                tools.push(tool.clone().into());
            }
            let param = ModelInputParam{
                temperature: None,
                tools: Some(tools),
                messages: chat_history,
            };
            let mut stream_res = Box::pin(agent.stream_chat(param).await);
            while let Some(res) = stream_res.next().await {
                match res {
                    Ok(CommonConnectionContent::Content(text)) => {
                        yield Ok(StreamedChatResponse::Text(text));
                    }
                    Ok(CommonConnectionContent::ToolCall(tool_call)) => {
                        yield Ok(StreamedChatResponse::ToolCall(tool_call));
                    }
                    Ok(CommonConnectionContent::Reasoning(reasoning)) => {
                        yield Ok(StreamedChatResponse::Reasoning(reasoning));
                    }
                    Ok(CommonConnectionContent::FinishReason(_)) => {
                        break;
                    }
                    Err(e) => {
                        yield Err(anyhow::anyhow!(e));
                        break;
                    }
                }
            }
        }
    }

    // 执行工具循环调用
    async fn tool_call(&mut self, tool_calls: Vec<ToolCall>, remain_loop: usize, messages: &mut Vec<ModelMessage>, tools: &Vec<Tool>)->anyhow::Result<()> {
        if remain_loop <= 0 || tool_calls.len() <= 0 {
            return Ok(());
        }
        
        // 创建运行时来执行异步操作
        let rt = Runtime::new().map_err(|e| anyhow::anyhow!("Failed to create runtime: {}", e))?;
        
        messages.push(ModelMessage::assistant("".into(), "".into(), tool_calls.clone()));
        for tool_call in tool_calls {
            // 解析工具调用参数
            let arguments: Value = serde_json::from_str(&tool_call.function.arguments)
                .unwrap_or_else(|_| Value::Object(serde_json::Map::new()));
            // 调用工具（阻塞执行）
            let tool_result = rt.block_on(McpManager::global().call_tool(
                &tool_call.function.name,
                &arguments
            ));
            println!("{:?}", tool_result);
            // 将工具调用结果添加到上下文
            if let Ok(result) = tool_result {
                messages.push(ModelMessage::tool(result, tool_call.clone()));
            } else if let Err(e) = tool_result {
                messages.push(ModelMessage::tool(
                    format!("工具调用失败: {}", e),
                    tool_call.clone()
                ));
            }
        }
        
        // 阻塞执行聊天
        let (res, tool_calls) = self.chat_internal(ModelInputParam{
            temperature: None,
            tools: Some(tools.clone()),
            messages: messages.clone(),
        }).await?;
        let mut tool_calls = Vec::new();
        for msg in res {
            if msg.tool_calls.is_none() {
                continue;
            }
            let mut calls = msg.tool_calls.unwrap();
            tool_calls.append(&mut calls);
        }
        
        // 递归调用（非异步）
        Box::pin(self.tool_call(tool_calls, remain_loop - 1, messages, tools)).await
    }

    async fn chat_internal(&self, param: ModelInputParam)->anyhow::Result<(Vec<ModelMessage>, Vec<ToolCall>)> {
        let res = self.agent.chat(param).await.map_err(|e| anyhow::anyhow!(e))?;
        let mut tool_calls = Vec::new();
        let content = String::new();
        let think = String::new();
        for ctx in res.iter() {
            if let CommonConnectionContent::ToolCall(tool) = ctx {
                tool_calls.push(tool.clone());
            }
        }
        let mut res = Vec::new();
        res.push(ModelMessage::assistant(content, think, tool_calls.clone()));
        Ok((res, tool_calls))
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use futures::{StreamExt, pin_mut};
    use super::*;

    #[tokio::test]
    async fn test_chat_streaming() -> Result<(), Box<dyn std::error::Error>> {
        let client = ChatClient::new("".to_string(), "".into(), vec![], 3);
        let stream = client.stream_chat("测试消息", vec![]);
        pin_mut!(stream);

        println!("开始接收流式响应:");
        while let Some(result) = stream.next().await {
            match result {
                Ok(StreamedChatResponse::Text(text)) => {
                    print!("{}", text);
                    std::io::stdout().flush()?;
                }
                Ok(StreamedChatResponse::ToolCall(tool_call)) => {
                    println!("\nTool Call: {:?}", tool_call);
                }
                Ok(StreamedChatResponse::Reasoning(reasoning)) => {
                    print!("\nThinking: {}", reasoning);
                    std::io::stdout().flush()?;
                }
                Err(e) => {
                    eprintln!("\n接收错误: {}", e);
                    break;
                }
            }
        }
        println!("\n流式响应结束");
        Ok(())
    }
}
