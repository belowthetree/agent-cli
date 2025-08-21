use serde::{Deserialize, Serialize};
use rmcp::model::Tool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMessage {
    pub role: String,
    pub content: String,
    pub think: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub tool_call_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

impl ModelMessage {
    pub fn user(content: String)->Self {
        Self {
            role: "user".into(),
            content,
            think: "".into(),
            name: "".into(),
            tool_call_id: "".into(),
            tool_calls: None,
        }
    }

    pub fn assistant(content: String, think: String, tool_calls: Vec<ToolCall>)->Self {
        Self {
            role: "assistant".into(),
            content,
            think: "".into(),
            name: "".into(),
            tool_call_id: "".into(),
            tool_calls: Some(tool_calls),
        }
    }

    pub fn system(content: String)->Self {
        Self {
            role: "system".into(),
            content,
            think: "".into(),
            name: "".into(),
            tool_call_id: "".into(),
            tool_calls: None,
        }
    }

    pub fn tool(content: String, tool: ToolCall)->Self {
        Self {
            role: "tool".into(),
            content,
            think: "".into(),
            name: tool.function.name,
            tool_call_id: tool.id,
            tool_calls: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInputParam {
    pub temperature: Option<f64>,
    pub tools: Option<Vec<Tool>>,
    pub messages: Vec<ModelMessage>,
}

fn _default_tool_call_function() -> ToolCallFunction {
    ToolCallFunction::new()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    #[serde(default)]
    pub index: usize,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub r#type: String,
    #[serde(default = "_default_tool_call_function")]
    pub function: ToolCallFunction,
}

impl ToolCall {
    #[allow(unused)]
    pub fn new() -> Self {
        Self {
            index: 0,
            id: String::new(),
            r#type: String::new(),
            function: ToolCallFunction::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFunction {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub arguments: String,
}

impl ToolCallFunction {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            arguments: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResponse {
    pub role: String,
    pub content: String,
    pub reasoning_content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub finish_reason: Option<String>,
}
