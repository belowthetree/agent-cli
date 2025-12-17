use log::info;

use crate::client::chat_client::ChatClient;
use crate::mcp::McpTool;
use crate::model::param::{ModelMessage, ToolCall};

#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub enum EChatState {
    // 正常状态
    Idle,
    // 正在接收模型消息
    Running,
    // 等待工具执行确认
    WaitingToolConfirm,
    // 等待工具调用，这个时候说明已经确认过了
    WaitingToolUse,
    // 等待继续对话确认
    WaitingTurnConfirm,
}

/// Chat 状态管理模块
/// 负责管理聊天状态、上下文和配置
#[derive(Clone)]
pub struct ChatState {
    pub client: ChatClient,
    pub context: Vec<ModelMessage>,
    cancel_token: tokio_util::sync::CancellationToken,
    state: EChatState,
    /// token限制
    max_tokens: Option<u32>,
    /// 是否在工具执行前询问用户确认
    ask_before_tool_execution: bool,
    /// 对话轮次统计
    conversation_turn_count: usize,
}

impl ChatState {
    /// 创建新的 ChatState
    pub fn new(
        client: ChatClient,
        context: Vec<ModelMessage>,
        max_tokens: Option<u32>,
        ask_before_tool_execution: bool,
    ) -> Self {
        Self {
            client,
            context,
            cancel_token: tokio_util::sync::CancellationToken::new(),
            state: EChatState::Idle,
            max_tokens,
            ask_before_tool_execution,
            conversation_turn_count: 0,
        }
    }

    /// 检查是否正在运行
    pub fn get_state(&self)->EChatState {
        self.state.clone()
    }

    pub fn set_state(&mut self, state: EChatState) {
        info!("设置状态 {:?}", state);
        self.state = state;
    }

    /// 获取取消令牌的副本
    pub fn get_cancel_token(&self) -> tokio_util::sync::CancellationToken {
        self.cancel_token.clone()
    }

    /// 设置工具
    pub fn set_tools(&mut self, tools: Vec<McpTool>) {
        self.client.tools(tools);
    }

    /// 检查是否有待处理的工具调用
    pub fn is_remain_tool_call(&self) -> bool {
        if let Some(last) = self.context.last() {
            if let Some(tools) = &last.tool_calls {
                return !tools.is_empty();
            }
        }
        false
    }

    /// 检查是否需要询问用户确认工具调用
    pub fn should_tool_confirmation(&self) -> bool {
        self.ask_before_tool_execution
    }

    /// 添加消息到上下文（支持批处理）
    pub fn add_message(&mut self, msg: ModelMessage) {
        self.context.push(msg);
    }

    /// 增加对话轮次计数
    pub fn increment_conversation_turn(&mut self) {
        self.conversation_turn_count += 1;
    }

    /// 重置对话轮次计数
    pub fn reset_conversation_turn(&mut self) {
        self.conversation_turn_count = 0;
    }

    /// 获取当前对话轮次统计
    pub fn get_conversation_turn_info(&self) -> usize {
        self.conversation_turn_count
    }

    /// 检查token使用是否超过限制
    /// 返回true表示超过限制，应该停止生成
    pub fn check_token_limit(&self, new_usage: Option<&crate::connection::TokenUsage>) -> bool {
        if let Some(max_tokens) = self.max_tokens {
            // 计算当前上下文的总token使用量
            let mut total_tokens = 0;
            if let Some(last) = self.context.last() {
                if let Some(usage) = &last.token_usage {
                    total_tokens = usage.total_tokens;
                }
            }
            
            // 如果提供了新的使用量，加上它
            if let Some(usage) = new_usage {
                total_tokens += usage.total_tokens;
            }
            
            // 检查是否超过限制
            if total_tokens > max_tokens {
                log::warn!("Token使用超过限制: {}/{} tokens", total_tokens, max_tokens);
                return true;
            }
        }
        false
    }

    /// 获取上下文
    pub fn context(&self) -> &Vec<ModelMessage> {
        &self.context
    }

    /// 获取可变的上下文
    pub fn context_mut(&mut self) -> &mut Vec<ModelMessage> {
        &mut self.context
    }

    /// 获取客户端
    pub fn client(&self) -> &ChatClient {
        &self.client
    }

    pub fn get_tool_calls(&self)->Vec<ToolCall> {
        if let Some(last) = self.context().last() {
            if let Some(tools) = &last.tool_calls {
                tools.clone()
            } else {
                vec![]
            }
        } else {
            vec![]
        }
    }
}
