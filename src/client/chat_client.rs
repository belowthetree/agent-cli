use std::sync::Arc;
use async_stream::stream;
use futures::{Stream, StreamExt};
use rig::{agent::Agent, client::{CompletionClient, ProviderClient, ProviderValue}, completion::Chat, message::{Message, ToolCall}, providers::deepseek::{self, CompletionModel}, streaming::StreamingChat};
use rig::streaming::StreamedAssistantContent;

pub struct ChatClient {
    pub agent: Arc<Agent<CompletionModel>>
}

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
    pub fn new(key: String) -> Self {
        let agent = deepseek::Client::from_val(ProviderValue::Simple(key))
            .agent("deepseek-chat")
            .preamble("你是一个优秀的AI助手，你善于用自身的知识和工具解决用户的需求")
            .temperature(0.6)
            .build();
        Self {
            agent: Arc::new(agent)
        }
    }

    pub async fn chat(&self, prompt: &str, chat_history: Vec<Message>) -> ChatResult {
        let res = self.agent.chat(prompt, chat_history).await.map_err(|e| anyhow::anyhow!(e)).unwrap();
        ChatResult {
            content: res,
            tool_calls: vec![],
            think: "".to_string(),
        }
    }


    pub fn stream_chat(&self, prompt: &str, chat_history: Vec<Message>)-> impl Stream<Item = Result<StreamedChatResponse, anyhow::Error>> + '_ {
        let prompt = prompt.to_string();
        let agent = self.agent.clone();
        stream! {
            let stream_res = agent.stream_chat(&prompt, chat_history).await;
            match stream_res {
                Ok(mut stream) => {
                    while let Some(res) = stream.next().await {
                        match res {
                            Ok(StreamedAssistantContent::Text(text)) => {
                                yield Ok(StreamedChatResponse::Text(text.text));
                            }
                            Ok(StreamedAssistantContent::ToolCall(tool_call)) => {
                                yield Ok(StreamedChatResponse::ToolCall(tool_call));
                            }
                            Ok(StreamedAssistantContent::Reasoning(reasoning)) => {
                                yield Ok(StreamedChatResponse::Reasoning(reasoning.reasoning));
                            }
                            Ok(StreamedAssistantContent::Final(_)) => {
                                break;
                            }
                            Err(e) => {
                                yield Err(anyhow::anyhow!(e));
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    yield Err(anyhow::anyhow!(e));
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
        let client = ChatClient::new("".to_string());
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
