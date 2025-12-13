//! 远程通信的协议定义。
//! 
//! 定义远程客户端与服务器之间通信的消息格式。

use serde::{Deserialize, Serialize};
use std::fmt;

/// 可以从远程客户端发送的输入类型。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InputType {
    /// 纯文本输入
    Text(String),
    /// Base64 编码的图像数据，带有可选的 MIME 类型
    Image {
        data: String,  // base64 encoded
        mime_type: Option<String>,
    },
    /// 结构化的指令/命令
    Instruction {
        command: String,
        parameters: serde_json::Value,
    },
    /// 带有内容类型的文件附件
    File {
        filename: String,
        content_type: String,
        data: String,  // base64 encoded
    },
    /// 多种输入类型的组合
    Multi(Vec<InputType>),
    /// 获取内置指令列表
    GetCommands,
    /// 中断当前正在进行的模型输出
    Interrupt,
    /// 重新生成最后的回复
    Regenerate,
}

impl InputType {
    /// 从输入中提取文本内容。
    /// 对于非文本输入，返回描述性字符串。
    pub fn to_text(&self) -> String {
        match self {
            InputType::Text(text) => text.clone(),
            InputType::Image { data: _, mime_type } => {
                format!("[Image: {}]", mime_type.as_deref().unwrap_or("unknown"))
            }
            InputType::Instruction { command, parameters } => {
                format!("[Instruction: {} with params: {}]", command, parameters)
            }
            InputType::File { filename, content_type, data: _ } => {
                format!("[File: {} ({})]", filename, content_type)
            }
            InputType::Multi(inputs) => {
                let parts: Vec<String> = inputs.iter().map(|i| i.to_text()).collect();
                parts.join(" + ")
            }
            InputType::GetCommands => "[GetCommands]".to_string(),
            InputType::Interrupt => "[Interrupt]".to_string(),
            InputType::Regenerate => "[Regenerate]".to_string(),
        }
    }
}

impl fmt::Display for InputType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_text())
    }
}

/// 从远程客户端到服务器的请求。
#[derive(Debug, Serialize, Deserialize)]
pub struct RemoteRequest {
    /// 请求的唯一标识符
    pub request_id: String,
    /// 来自用户的输入数据
    pub input: InputType,
    /// 可选的配置覆盖
    pub config: Option<RequestConfig>,
    /// 是否流式传输响应
    pub stream: Option<bool>,
    /// 是否使用工具
    pub use_tools: Option<bool>,
}

/// 请求的配置覆盖。
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RequestConfig {
    /// 最大工具尝试次数
    pub max_tool_try: Option<usize>,
    /// 最大上下文长度
    pub max_context_num: Option<usize>,
    /// 最大令牌数
    pub max_tokens: Option<u32>,
    /// 是否在执行工具前询问
    pub ask_before_tool_execution: Option<bool>,
    /// 自定义提示/指令
    pub prompt: Option<String>,
}

/// 从服务器到远程客户端的响应。
#[derive(Debug, Serialize, Deserialize)]
pub struct RemoteResponse {
    /// 对应的请求 ID
    pub request_id: String,
    /// 响应内容
    pub response: ResponseContent,
    /// 可选的错误信息
    pub error: Option<String>,
    /// 令牌使用统计信息（如果可用）
    pub token_usage: Option<TokenUsage>,
}

/// 响应的内容。
#[derive(Debug, Serialize, Deserialize)]
pub enum ResponseContent {
    /// 文本响应
    Text(String),
    /// 文本块的流（用于流式响应）
    Stream(Vec<String>),
    /// 包含多个部分的组合响应
    Multi(Vec<ResponseContent>),
    /// 工具调用信息
    ToolCall {
        name: String,
        arguments: serde_json::Value,
    },
    /// 工具执行结果
    ToolResult {
        name: String,
        result: serde_json::Value,
    },
}

/// 令牌使用统计信息。
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

impl RemoteResponse {
    /// 创建一个错误响应。
    pub fn error(request_id: &str, error: &str) -> Self {
        Self {
            request_id: request_id.to_string(),
            response: ResponseContent::Text(String::new()),
            error: Some(error.to_string()),
            token_usage: None,
        }
    }
}
