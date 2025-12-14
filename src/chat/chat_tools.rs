use crate::client::tool_client;
use crate::model::param::{ModelMessage, ToolCall};
use futures::Stream;
use futures::StreamExt;

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
}
