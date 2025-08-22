use crate::model::param::ToolCall;

pub mod common;

#[derive(Debug)]
pub enum CommonConnectionContent {
    Content(String),
    Reasoning(String),
    ToolCall(ToolCall),
    FinishReason(String),
}