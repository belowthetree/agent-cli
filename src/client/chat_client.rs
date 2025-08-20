use async_stream::stream;
use futures::{Stream, StreamExt};

use crate::{connection::CommonConnectionContent, mcp::McpTool, model::{deepseek, param::{ModelInputParam, ModelMessage, ToolCall}, AgentModel}};

pub struct ChatClient {
    pub agent: deepseek::DeepseekModel,
    tools: Vec<McpTool>
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
    pub fn new(key: String, tools: Vec<McpTool>) -> Self {
        let agent = deepseek::DeepseekModel::new("https://api.deepseek.com/chat/completions".into(), "deepseek-chat".into(), key);
        Self {
            agent,
            tools,
        }
    }

    pub async fn chat(&self, prompt: &str, mut chat_history: Vec<ModelMessage>) -> ChatResult {
        let mut tools = Vec::new();
        for tool in self.tools.iter() {
            tools.push(tool.clone().into());
        }
        chat_history.push(ModelMessage::user(prompt.to_string()));
        let param = ModelInputParam{
            system: None,
            temperature: None,
            tools: Some(tools),
            messages: chat_history,
        };
        let res = self.agent.chat(param).await.map_err(|e| anyhow::anyhow!(e)).unwrap();
        let mut tool_calls = Vec::new();
        let content = String::new();
        let think = String::new();
        for ctx in res.iter() {
            if let CommonConnectionContent::ToolCall(tool) = ctx {
                tool_calls.push(tool.clone());
            }
        }
        ChatResult {
            content,
            tool_calls,
            think,
        }
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
                system: None,
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
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use futures::{StreamExt, pin_mut};
    use super::*;

    #[tokio::test]
    async fn test_chat_streaming() -> Result<(), Box<dyn std::error::Error>> {
        let client = ChatClient::new("".to_string(), vec![]);
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
