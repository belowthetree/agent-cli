use serde::{Deserialize, Serialize};
use thiserror::Error;

/// ACP错误类型
#[derive(Error, Debug)]
pub enum AcpError {
    #[error("JSON解析错误: {0}")]
    ParseError(#[from] serde_json::Error),
    
    #[error("无效的请求")]
    InvalidRequest,
    
    #[error("方法不存在: {0}")]
    MethodNotFound(String),
    
    #[error("无效的参数: {0}")]
    InvalidParams(String),
    
    #[error("内部错误: {0}")]
    InternalError(String),
    
    #[error("会话不存在: {0}")]
    SessionNotFound(String),
    
    #[error("传输层错误: {0}")]
    TransportError(String),
    
    #[error("超时")]
    Timeout,
}

pub type AcpResult<T> = Result<T, AcpError>;

/// JSON-RPC 2.0 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// JSON-RPC 2.0 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: u64,
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 错误
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// JSON-RPC 2.0 通知
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    pub params: serde_json::Value,
}

/// 初始化参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeParams {
    pub protocol_version: String,
    pub client_info: ClientInfo,
}

/// 客户端信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

/// 初始化结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeResult {
    pub protocol_version: String,
    pub server_info: ServerInfo,
    pub capabilities: Capabilities,
}

/// 服务器信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

/// 能力信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub streaming: Option<bool>,
}

/// 创建会话参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionNewParams {
    pub cwd: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<Vec<McpServerConfig>>,
}

/// MCP服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
}

/// 创建会话结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionNewResult {
    pub session_id: String,
}

/// 发送提示参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionPromptParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub prompt: String,
}

/// 发送提示结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionPromptResult {
    pub success: bool,
}

/// 会话更新参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionUpdateParams {
    pub session_id: String,
    pub update: SessionUpdate,
}

/// 会话更新
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "sessionUpdate")]
pub enum SessionUpdate {
    AgentMessageChunk(AgentMessageChunk),
    AgentThoughtChunk(AgentThoughtChunk),
    ToolCall(ToolCallUpdate),
    Plan(PlanUpdate),
    AvailableCommands(AvailableCommandsUpdate),
    UserMessageChunk(UserMessageChunk),
}

/// Agent消息块
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessageChunk {
    pub content: Content,
}

/// Agent思考块
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentThoughtChunk {
    pub content: Content,
}

/// 内容
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Content {
    Text { text: String },
}

/// 工具调用更新
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallUpdate {
    pub tool_call_id: String,
    pub status: ToolCallStatus,
    pub title: String,
    pub kind: ToolCallKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_input: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Vec<ToolCallContent>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locations: Option<Vec<Location>>,
}

/// 工具调用状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolCallStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
}

/// 工具调用类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolCallKind {
    #[serde(rename = "read")]
    Read,
    #[serde(rename = "edit")]
    Edit,
    #[serde(rename = "execute")]
    Execute,
}

/// 工具调用内容
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolCallContent {
    Diff { diff: String },
    Content { content: serde_json::Value },
}

/// 位置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub path: String,
}

/// 计划更新
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanUpdate {
    pub entries: Vec<PlanEntry>,
}

/// 计划条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanEntry {
    pub content: String,
    pub status: PlanEntryStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<PlanEntryPriority>,
}

/// 计划条目状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlanEntryStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(rename = "completed")]
    Completed,
}

/// 计划条目优先级
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlanEntryPriority {
    #[serde(rename = "high")]
    High,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "low")]
    Low,
}

/// 可用命令更新
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableCommandsUpdate {
    pub commands: Vec<Command>,
}

/// 命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// 用户消息块
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMessageChunk {
    pub content: Content,
}

/// 结束本轮参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndTurnParams {
    pub session_id: String,
}

/// 权限请求参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequestParams {
    pub session_id: String,
    pub options: Vec<PermissionOption>,
    pub tool_call: PermissionToolCall,
}

/// 权限选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionOption {
    pub option_id: String,
    pub name: String,
    pub kind: PermissionOptionKind,
}

/// 权限选项类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PermissionOptionKind {
    #[serde(rename = "allow_once")]
    AllowOnce,
    #[serde(rename = "allow_always")]
    AllowAlways,
    #[serde(rename = "reject_once")]
    RejectOnce,
    #[serde(rename = "reject_always")]
    RejectAlways,
}

/// 权限工具调用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionToolCall {
    pub tool_call_id: String,
    pub title: String,
    pub kind: ToolCallKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Vec<ToolCallContent>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locations: Option<Vec<Location>>,
}
