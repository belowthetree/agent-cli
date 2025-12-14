use crate::model::param::ToolCall;
use serde::{Deserialize, Serialize};

pub mod common;
pub mod cache;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    // 当前对话本地发送 token 数，也就是前面所有对话加上本次用户输出
    pub prompt_tokens: u32,
    // 本次对话中模型输出 token 数
    pub completion_tokens: u32,
    // 到当前对话为止的总 token 数，即前面所有对话之和
    pub total_tokens: u32,
}

#[derive(Debug, Clone)]
pub enum CommonConnectionContent {
    Content(String),
    Reasoning(String),
    ToolCall(ToolCall),
    FinishReason(String),
    TokenUsage(TokenUsage),
}
