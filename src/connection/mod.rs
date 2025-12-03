use crate::model::param::ToolCall;
use serde::{Deserialize, Serialize};

pub mod common;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug)]
pub enum CommonConnectionContent {
    Content(String),
    Reasoning(String),
    ToolCall(ToolCall),
    FinishReason(String),
    TokenUsage(TokenUsage),
}
