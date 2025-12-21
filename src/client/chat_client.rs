use async_stream::stream;
use futures::{Stream, StreamExt};
use log::{info};
use rmcp::model::Tool;

use crate::{connection::{CommonConnectionContent, TokenUsage}, mcp::{McpTool}, model::{deepseek, param::{ModelInputParam, ModelMessage}, AgentModel}};

#[derive(Clone)]
pub struct ChatClient {
    pub agent: deepseek::DeepseekModel,
    tools: Vec<Tool>,
}

impl ChatClient {
    pub fn new(key: String, url: String, model: String, tools: Vec<McpTool>) -> Self {
        let agent = deepseek::DeepseekModel::new(url, model, key);
        let mut client = Self {
            agent,
            tools: vec![],
        };
        info!("初始化工具: {:?}", tools);
        client.tools(tools);
        client
    }

    pub async fn get_token_limit(&self)->u32 {
        self.agent.get_token_limit()
    }

    pub fn tools(&mut self, tools: Vec<McpTool>) {
        self.tools.clear();
        for tool in tools {
            self.tools.push(tool.get_tool());
        }
    }

    /// 获取工具列表的引用，避免不必要的克隆
    fn get_tools_ref(&self) -> Option<&Vec<Tool>> {
        if self.tools.is_empty() {
            None
        } else {
            Some(&self.tools)
        }
    }

    /// 构建模型输入参数
    fn build_model_input(&self, messages: Vec<ModelMessage>) -> ModelInputParam {
        ModelInputParam {
            temperature: None,
            tools: self.get_tools_ref().cloned(),
            messages,
        }
    }

    pub fn chat2(&self, messages: Vec<ModelMessage>)->impl Stream<Item = Result<ModelMessage, anyhow::Error>> + '_ {
        info!("chat2 开始，消息数量: {}", messages.len());
        let param = self.build_model_input(messages);
        
        stream! {
            let answer = match self.agent.chat(param).await {
                Ok(answer) => answer,
                Err(e) => {
                    yield Err(anyhow::anyhow!("聊天请求失败: {}", e));
                    return;
                }
            };
            
            info!("chat2 收到答复，内容块数量: {}", answer.len());
            let mut tool_calls = Vec::new();
            let mut content = String::new();
            let mut think = String::new();
            let mut token_usage: Option<TokenUsage> = None;
            
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
                    CommonConnectionContent::TokenUsage(usage) => {
                        info!("Token 使用情况: prompt_tokens={}, completion_tokens={}, total_tokens={}", 
                            usage.prompt_tokens, usage.completion_tokens, usage.total_tokens);
                        token_usage = Some(usage.clone());
                    }
                    _ => {}
                }
            }
            
            let mut msg = ModelMessage::assistant(content, think, tool_calls);
            msg.token_usage = token_usage;
            yield Ok(msg);
        }
    }

    // 返回增量
    pub fn stream_chat(&self, messages: Vec<ModelMessage>)-> impl Stream<Item = Result<ModelMessage, anyhow::Error>> + '_ {
        let agent = self.agent.clone();
        let param = self.build_model_input(messages);
        
        stream! {
            info!("stream chat 开始，参数: {:?}", param);
            let mut stream_res = Box::pin(agent.stream_chat(param).await);
            
            while let Some(res) = stream_res.next().await {
                match res {
                    Ok(CommonConnectionContent::Content(text)) => {
                        yield Ok(ModelMessage::assistant(text, "", vec![]));
                    }
                    Ok(CommonConnectionContent::ToolCall(tool_call)) => {
                        yield Ok(ModelMessage::assistant("", "", vec![tool_call]));
                    }
                    Ok(CommonConnectionContent::Reasoning(reasoning)) => {
                        yield Ok(ModelMessage::assistant("", reasoning, vec![]));
                    }
                    Ok(CommonConnectionContent::FinishReason(reason)) => {
                        info!("流式聊天完成，原因: {}", reason);
                    }
                    Ok(CommonConnectionContent::TokenUsage(usage)) => {
                        info!("Token 使用情况: prompt_tokens={}, completion_tokens={}, total_tokens={}", 
                            usage.prompt_tokens, usage.completion_tokens, usage.total_tokens);
                        yield Ok(ModelMessage::token(usage));
                    }
                    Err(e) => {
                        yield Err(anyhow::anyhow!("流式响应错误: {}", e));
                        break;
                    }
                }
            }
        }
    }
}
