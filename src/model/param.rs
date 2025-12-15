use std::borrow::Cow;
use serde::{Deserialize, Serialize};
use rmcp::model::Tool;
use crate::connection::TokenUsage;

/// 辅助函数用于检查 Cow<'static, str> 是否为空
fn cow_is_empty(cow: &Cow<'static, str>) -> bool {
    cow.is_empty()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelMessage {
    pub role: Cow<'static, str>,
    pub content: Cow<'static, str>,
    pub think: Cow<'static, str>,
    #[serde(skip_serializing_if = "cow_is_empty")]
    pub name: Cow<'static, str>,
    #[serde(skip_serializing_if = "cow_is_empty")]
    pub tool_call_id: Cow<'static, str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_usage: Option<TokenUsage>,
}

impl ModelMessage {
    pub fn user<S: Into<Cow<'static, str>>>(content: S) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
            think: "".into(),
            name: "".into(),
            tool_call_id: "".into(),
            tool_calls: None,
            token_usage: None,
        }
    }

    pub fn assistant<S1: Into<Cow<'static, str>>, S2: Into<Cow<'static, str>>>(
        content: S1,
        think: S2,
        tool_calls: Vec<ToolCall>,
    ) -> Self {
        let tool_calls = if !tool_calls.is_empty() { Some(tool_calls) } else { None };
        Self {
            role: "assistant".into(),
            content: content.into(),
            think: think.into(),
            name: "".into(),
            tool_call_id: "".into(),
            tool_calls,
            token_usage: None,
        }
    }

    pub fn system<S: Into<Cow<'static, str>>>(content: S) -> Self {
        Self {
            role: "system".into(),
            content: content.into(),
            think: "".into(),
            name: "".into(),
            tool_call_id: "".into(),
            tool_calls: None,
            token_usage: None,
        }
    }

    pub fn tool<S: Into<Cow<'static, str>>>(content: S, tool: ToolCall) -> Self {
        Self {
            role: "tool".into(),
            content: content.into(),
            think: "".into(),
            name: tool.function.name.into(),
            tool_call_id: tool.id.into(),
            tool_calls: None,
            token_usage: None,
        }
    }

    pub fn token(token_usage: TokenUsage) -> Self {
        Self {
            role: "system".into(),
            content: "".into(),
            think: "".into(),
            name: "".into(),
            tool_call_id: "".into(),
            tool_calls: None,
            token_usage: Some(token_usage),
        }
    }

    pub fn add_tool(&mut self, tool: ToolCall) {
        if self.tool_calls.is_none() {
            self.tool_calls = Some(vec![]);
        }
        if let Some(tools) = &mut self.tool_calls {
            tools.push(tool);
        }
    }

    pub fn add_content<S: Into<Cow<'static, str>>>(&mut self, content: S) {
        let new_content = content.into();
        if let Cow::Owned(mut owned) = self.content.clone() {
            owned.push_str(&new_content);
            self.content = Cow::Owned(owned);
        } else {
            let mut owned = self.content.to_string();
            owned.push_str(&new_content);
            self.content = Cow::Owned(owned);
        }
    }

    pub fn add_think<S: Into<Cow<'static, str>>>(&mut self, think: S) {
        let new_think = think.into();
        if let Cow::Owned(mut owned) = self.think.clone() {
            owned.push_str(&new_think);
            self.think = Cow::Owned(owned);
        } else {
            let mut owned = self.think.to_string();
            owned.push_str(&new_think);
            self.think = Cow::Owned(owned);
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
