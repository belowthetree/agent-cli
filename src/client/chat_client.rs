use async_recursion::async_recursion;
use async_stream::stream;
use futures::{Stream, StreamExt};
use log::{info, warn};
use rmcp::model::Tool;
use serde_json::Value;
use tokio::runtime::Runtime;

use crate::{connection::CommonConnectionContent, mcp::{McpManager, McpTool}, model::{deepseek, param::{ModelInputParam, ModelMessage, ToolCall}, AgentModel}};

pub struct ChatClient {
    pub agent: deepseek::DeepseekModel,
    system: String,
    tools: Vec<Tool>,
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
        let agent = deepseek::DeepseekModel::new("https://api.deepseek.com".into(), "deepseek-chat".into(), key);
        let mut client = Self {
            agent,
            system,
            tools: vec![],
            max_tool_loop,
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

    pub fn max_try(&mut self, max_tool_loop: usize) {
        self.max_tool_loop = max_tool_loop;
    }

    pub async fn chat(&mut self, prompt: &str, chat_history: Vec<ModelMessage>) -> anyhow::Result<Vec<ModelMessage>> {
        let mut tools = Vec::new();
        for tool in self.tools.iter() {
            tools.push(tool.clone().into());
        }
        self.chat_with_tools(prompt, chat_history, &tools).await
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

    pub async fn chat_with_tools(&mut self, prompt: &str, mut chat_history: Vec<ModelMessage>, tools: &Vec<Tool>)->anyhow::Result<Vec<ModelMessage>> {
        if self.system.len() > 0 {
            chat_history.push(ModelMessage::system(self.system.to_string()));
        }
        if prompt.len() > 0 {
            chat_history.push(ModelMessage::user(prompt.to_string()));
        }
        let ts = if tools.len() > 0 {Some(tools.clone())} else {None};
        let param = ModelInputParam{
            temperature: None,
            tools: ts,
            messages: chat_history.clone(),
        };
        let (res, tool_calls) = self.get_model_answer(param).await?;
        for msg in res.iter() {
            chat_history.push(msg.clone());
        }
        if tool_calls.len() > 0 {
            self.tool_call(tool_calls, self.max_tool_loop, &mut chat_history, tools).await?;
        }
        Ok(chat_history)
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

    #[async_recursion]
    // 执行工具循环调用
    async fn tool_call(&mut self, tool_calls: Vec<ToolCall>, remain_loop: usize, messages: &mut Vec<ModelMessage>, tools: &Vec<Tool>)->anyhow::Result<()> {
        info!("tool_call {}", remain_loop);
        let should_break = remain_loop <= 0 || tool_calls.len() <= 0;
        for tool_call in tool_calls {
            // 解析工具调用参数
            let arguments: Value = serde_json::from_str(&tool_call.function.arguments)
                .unwrap_or_else(|_| Value::Object(serde_json::Map::new()));
            // 调用工具（阻塞执行）
            let tool_result = McpManager::global().call_tool(
                &tool_call.function.name,
                &arguments
            ).await;
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
        // 工具调用可以无限，但是聊天不能
        if should_break {
            return Ok(());
        }
        info!("工具调用完成，返回工具结果给对话 {}", self.system);
        // 阻塞执行聊天
        let (res, tool_calls) = self.get_model_answer(ModelInputParam{
            temperature: None,
            tools: Some(tools.clone()),
            messages: messages.clone(),
        }).await?;
        for msg in res {
            messages.push(msg);
        }
        // 工具调用记入历史
        messages.push(ModelMessage::assistant("".into(), "".into(), tool_calls.clone()));
        // 递归调用（非异步）
        Box::pin(self.tool_call(tool_calls, remain_loop - 1, messages, tools)).await
    }

    pub fn call_tool(&self, tool_calls: Vec<ToolCall>)-> impl Stream<Item = Result<ModelMessage, anyhow::Error>> {
        stream! {
            if tool_calls.len() <= 0 {
                warn!("call_tool 工具数量不能为 0");
                yield Err(anyhow::anyhow!("call_tool 工具数量不能为 0"));
                return;
            }
            for tool_call in tool_calls {
                // 解析工具调用参数
                let arguments: Value = serde_json::from_str(&tool_call.function.arguments)
                    .unwrap_or_else(|_| Value::Object(serde_json::Map::new()));
                // 调用工具（阻塞执行）
                let tool_result = McpManager::global().call_tool(
                    &tool_call.function.name,
                    &arguments
                ).await;
                // 将工具调用结果添加到上下文
                if let Ok(result) = tool_result {
                    yield Ok(ModelMessage::tool(result, tool_call.clone()));
                }
                else if let Err(e) = tool_result {
                    yield Ok(ModelMessage::tool(
                        format!("工具调用失败：{}", e),
                        tool_call.clone()
                    ));
                }
            }
        }
    }

    async fn get_model_answer(&self, param: ModelInputParam)->anyhow::Result<(Vec<ModelMessage>, Vec<ToolCall>)> {
        info!("调用 get_model_answer {:?}", param.messages);
        let answer = self.agent.chat(param).await.map_err(|e| anyhow::anyhow!(e))?;
        info!("答复：{:?}", answer);
        let mut tool_calls = Vec::new();
        let mut content = String::new();
        let mut think = String::new();
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
        // 将模型、工具回复写入到上下文
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
