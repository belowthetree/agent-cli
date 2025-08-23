use async_stream::stream;
use futures::{Stream, StreamExt};
use log::{info};
use rmcp::model::Tool;

use crate::{connection::CommonConnectionContent, mcp::{McpTool}, model::{deepseek, param::{ModelInputParam, ModelMessage}, AgentModel}};

pub struct ChatClient {
    pub agent: deepseek::DeepseekModel,
    tools: Vec<Tool>,
}

impl ChatClient {
    pub fn new(key: String, tools: Vec<McpTool>) -> Self {
        let agent = deepseek::DeepseekModel::new("https://api.deepseek.com".into(), "deepseek-chat".into(), key);
        // let agent = deepseek::DeepseekModel::new("http://localhost:11434/v1".into(), "qwen3:4b".into(), key);
        let mut client = Self {
            agent,
            tools: vec![],
        };
        client.tools(tools);
        client
    }

    pub fn tools(&mut self, tools: Vec<McpTool>) {
        self.tools.clear();
        for tool in tools {
            self.tools.push(tool.get_tool());
        }
    }

    pub fn chat2(&self, messages: Vec<ModelMessage>)->impl Stream<Item = Result<ModelMessage, anyhow::Error>> + '_ {
        info!("chat2 开始 {:?}", messages);
        let tools = if self.tools.len() > 0 {Some(self.tools.clone())} else {None};
        let param = ModelInputParam{
            temperature: None,
            tools,
            messages,
        };
        stream! {
            let answer = self.agent.chat(param).await.map_err(|e| anyhow::anyhow!(e))?;
            info!("chat2 答复：{:?}", answer);
            let mut tool_calls = Vec::new();
            let mut content = String::new();
            let mut think = String::new();
            // 非流式请求，工具调用、回复、思维链在同一次回复里
            for ctx in answer.iter() {
                match ctx {
                    CommonConnectionContent::ToolCall(tool) => {
                        tool_calls.push(tool.clone());
                    }
                    CommonConnectionContent::Content(ct) => {
                        content = ct.clone();
                    }
                    CommonConnectionContent::Reasoning(reason) => {
                        think = reason.clone();
                    }
                    _ => {}
                }
            }
            yield Ok(ModelMessage::assistant(content, think, tool_calls.clone()));
        }
    }

    // 返回增量
    pub fn stream_chat(&self, messages: Vec<ModelMessage>)-> impl Stream<Item = Result<ModelMessage, anyhow::Error>> + '_ {
        let agent = self.agent.clone();
        stream! {
            let tools = if self.tools.len() > 0 {Some(self.tools.clone())} else {None};
            let param = ModelInputParam{
                temperature: None,
                tools,
                messages,
            };
            info!("stream chat {:?}", param);
            let mut stream_res = Box::pin(agent.stream_chat(param).await);
            while let Some(res) = stream_res.next().await {
                match res {
                    Ok(CommonConnectionContent::Content(text)) => {
                        yield Ok(ModelMessage::assistant(text, "".into(), vec![]));
                    }
                    Ok(CommonConnectionContent::ToolCall(tool_call)) => {
                        yield Ok(ModelMessage::assistant("".into(), "".into(), vec![tool_call]));
                    }
                    Ok(CommonConnectionContent::Reasoning(reasoning)) => {
                        yield Ok(ModelMessage::assistant("".into(), reasoning, vec![]));
                    }
                    Ok(CommonConnectionContent::FinishReason(reason)) => {
                        info!("finish {}", reason);
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
    use futures::{StreamExt, pin_mut};
    use super::*;

    #[tokio::test]
    async fn test_chat_streaming() -> Result<(), Box<dyn std::error::Error>> {
        let client = ChatClient::new("".to_string(), vec![]);
        let stream = client.stream_chat(vec![ModelMessage::user("测试消息".into())]);
        pin_mut!(stream);

        println!("开始接收流式响应:");
        let mut msg = ModelMessage::assistant("".into(), "".into(), vec![]);
        while let Some(result) = stream.next().await {
            if let Ok(res) = result {
                if msg.think.len() < res.think.len() {
                    print!("{}", res.think.split_at(msg.think.len()).1);
                    msg.think = res.think;
                }
                if msg.content.len() < res.content.len() {
                    print!("{}", res.content.split_at(msg.content.len()).1);
                    msg.content = res.content;
                }
            }
        }
        println!("\n流式响应结束");
        Ok(())
    }
}
