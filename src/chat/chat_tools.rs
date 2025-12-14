use crate::client::tool_client;
use crate::model::param::{ModelMessage, ToolCall};
use futures::Stream;
use futures::StreamExt;

use super::chat_state::ChatState;

/// Chat 工具调用处理模块
/// 负责处理工具调用和执行
pub struct ChatTools;

impl ChatTools {
    /// 调用工具并返回结果流
    pub fn call_tool(
        tool_calls: Vec<ToolCall>,
        cancel_token: tokio_util::sync::CancellationToken,
    ) -> impl Stream<Item = anyhow::Result<ModelMessage>> + 'static {
        async_stream::stream! {
            if tool_calls.is_empty() {
                return;
            }
            let caller = tool_client::ToolClient;
            let stream = caller.call(tool_calls);
            futures::pin_mut!(stream);
            while let Some(res) = stream.next().await {
                if cancel_token.is_cancelled() {
                    return;
                }
                yield res;
            }
        }
    }

    /// 处理工具调用确认逻辑
    pub fn handle_tool_confirmation(
        state: &mut ChatState,
        _tool_calls: Vec<ToolCall>,
    ) -> Vec<ModelMessage> {
        let tool_responses = Vec::new();
        
        // 检查是否需要询问用户确认
        if state.should_ask_for_tool_confirmation() {
            // 设置等待确认状态
            state.set_waiting_tool_confirmation(true);
            // 不执行工具调用，等待用户确认
            return tool_responses;
        } else {
            // 不需要询问，直接执行工具调用
            // 注意：实际执行需要在异步上下文中进行
            // 这里只返回空响应，实际执行由调用者处理
            tool_responses
        }
    }

    /// 执行工具调用（在确认后）
    pub async fn execute_tools(
        tool_calls: Vec<ToolCall>,
        cancel_token: tokio_util::sync::CancellationToken,
    ) -> Vec<anyhow::Result<ModelMessage>> {
        let mut results = Vec::new();
        let stream = Self::call_tool(tool_calls, cancel_token);
        futures::pin_mut!(stream);
        
        while let Some(result) = stream.next().await {
            results.push(result);
        }
        
        results
    }

    /// 拒绝工具调用并添加失败消息
    pub fn reject_tool_calls(state: &mut ChatState) {
        if let Some(last) = state.context().last().cloned() {
            if let Some(tools) = last.tool_calls {
                for tool in tools {
                    state.add_message(ModelMessage::tool("失败：用户拒绝", tool));
                }
            }
        }
        state.set_waiting_tool_confirmation(false);
    }
}
