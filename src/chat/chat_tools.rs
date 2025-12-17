use crate::chat::{Chat, StreamedChatResponse};
use crate::client::tool_client;
use crate::model::param::{ModelMessage, ToolCall};
use futures::{Stream, pin_mut};
use futures::StreamExt;
use log::info;

/// Chat 工具调用处理模块
/// 负责处理工具调用和执行
pub struct ChatTools;

impl ChatTools {
    pub fn handle_stream_tool(chat: &mut Chat, cancel_token: tokio_util::sync::CancellationToken)-> impl Stream<Item = anyhow::Result<StreamedChatResponse>> + '_ {
        async_stream::stream! {
            // 处理工具调用
            let tool_calls = chat.state.get_tool_calls();
            info!("工具数 {:?}", tool_calls);
            if tool_calls.len() > 0 {
                // 不需要询问，直接执行工具调用
                let mut tool_responses = Vec::new();
                {
                    let stream = ChatTools::call_tool(tool_calls, cancel_token.clone());
                    pin_mut!(stream);
                    while let Some(res) = stream.next().await {
                        match res {
                            Ok(res) => {
                                yield Ok(StreamedChatResponse::ToolResponse(res.clone()));
                                tool_responses.push(res);
                            }
                            Err(e) => {
                                yield Err(e);
                            }
                        }
                    }
                }
                for response in tool_responses {
                    chat.state.add_message(response);
                }
            }
        }
    }
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
                    info!("工具取消");
                    return;
                }
                yield res;
            }
        }
    }
}
