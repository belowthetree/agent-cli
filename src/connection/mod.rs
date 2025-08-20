use crate::model::param::ToolCall;

pub mod common;

pub enum CommonConnectionContent {
    Content(String),
    Reasoning(String),
    ToolCall(ToolCall),
    FinishReason(String),
}