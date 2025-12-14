use crate::client::chat_client::ChatClient;
use crate::mcp::McpTool;
use crate::model::param::ModelMessage;

/// Chat 状态管理模块
/// 负责管理聊天状态、上下文和配置
#[derive(Clone)]
pub struct ChatState {
    pub client: ChatClient,
    pub context: Vec<ModelMessage>,
    max_tool_try: usize,
    cancel_token: tokio_util::sync::CancellationToken,
    running: bool,
    /// token限制
    max_tokens: Option<u32>,
    /// 是否在工具执行前询问用户确认
    ask_before_tool_execution: bool,
    /// 是否正在等待用户确认工具调用
    waiting_tool_confirmation: bool,
    /// 对话轮次统计
    conversation_turn_count: usize,
    /// 最大对话轮次数
    max_context_num: usize,
    /// 是否正在等待对话轮次确认
    waiting_context_confirmation: bool,
}

impl ChatState {
    /// 创建新的 ChatState
    pub fn new(
        client: ChatClient,
        context: Vec<ModelMessage>,
        max_tool_try: usize,
        max_tokens: Option<u32>,
        ask_before_tool_execution: bool,
        max_context_num: usize,
    ) -> Self {
        Self {
            client,
            context,
            max_tool_try,
            cancel_token: tokio_util::sync::CancellationToken::new(),
            running: false,
            max_tokens,
            ask_before_tool_execution,
            waiting_tool_confirmation: false,
            conversation_turn_count: 0,
            max_context_num,
            waiting_context_confirmation: false,
        }
    }

    /// 检查是否正在运行
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// 锁定状态（开始运行）
    pub fn lock(&mut self) {
        self.running = true;
    }

    /// 解锁状态（停止运行）
    pub fn unlock(&mut self) {
        self.running = false;
    }

    /// 取消操作
    pub fn cancel(&self) {
        self.cancel_token.cancel();
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
    pub fn is_waiting_tool(&self) -> bool {
        if let Some(last) = self.context.last() {
            if let Some(tools) = &last.tool_calls {
                return !tools.is_empty();
            }
        }
        false
    }

    /// 检查是否需要询问用户确认工具调用
    pub fn should_ask_for_tool_confirmation(&self) -> bool {
        self.ask_before_tool_execution
    }

    /// 设置等待工具确认状态
    pub fn set_waiting_tool_confirmation(&mut self, waiting: bool) {
        self.waiting_tool_confirmation = waiting;
    }

    /// 检查是否正在等待工具确认
    pub fn is_waiting_tool_confirmation(&self) -> bool {
        self.waiting_tool_confirmation
    }

    /// 确认工具调用
    pub fn confirm_tool_call(&mut self) {
        self.waiting_tool_confirmation = false;
    }

    /// 拒绝工具调用
    pub fn reject_tool_call(&mut self) {
        self.waiting_tool_confirmation = false;
    }

    /// 设置工具确认结果
    pub fn set_tool_confirmation_result(&mut self, approved: bool) {
        if approved {
            self.confirm_tool_call();
        } else {
            self.reject_tool_call();
        }
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

    /// 检查是否超过最大对话轮次
    pub fn is_over_context_limit(&self) -> bool {
        self.conversation_turn_count >= self.max_context_num
    }

    /// 设置等待对话轮次确认状态
    pub fn set_waiting_context_confirmation(&mut self, waiting: bool) {
        self.waiting_context_confirmation = waiting;
    }

    /// 检查是否正在等待对话轮次确认
    pub fn is_waiting_context_confirmation(&self) -> bool {
        self.waiting_context_confirmation
    }

    /// 获取当前对话轮次统计
    pub fn get_conversation_turn_info(&self) -> (usize, usize) {
        (self.conversation_turn_count, self.max_context_num)
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

    /// 获取最大工具尝试次数
    pub fn max_tool_try(&self) -> usize {
        self.max_tool_try
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
}
