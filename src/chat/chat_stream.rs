use async_stream::stream;
use futures::{pin_mut, Stream, StreamExt};
use log::info;

use crate::model::param::{ModelMessage, ToolCall};

use super::chat_state::ChatState;
use super::chat_tools::ChatTools;

/// 流式聊天响应类型
#[derive(Debug)]
pub enum StreamedChatResponse {
    Text(String),
    ToolCall(ToolCall),
    Reasoning(String),
    ToolResponse(ModelMessage),
    End,
}

/// Chat 流处理模块
/// 负责处理流式聊天响应
pub struct ChatStream;

impl ChatStream {
    /// 处理流式聊天
    pub fn handle_stream_chat(
        state: &mut ChatState,
    ) -> impl Stream<Item = Result<StreamedChatResponse, anyhow::Error>> + '_ {
        let cancel_token = state.get_cancel_token();
        state.lock();
        
        stream! {
            let mut count = state.max_tool_try();
            loop {
                let mut msg = ModelMessage::assistant("", "", vec![]);
                {
                    let stream = state.client().stream_chat(state.context().to_vec());
                    pin_mut!(stream);
                    // 接收模型输出
                    while let Some(res) = stream.next().await {
                        // 检查是否已取消
                        if cancel_token.is_cancelled() {
                            break;
                        }
                        info!("{:?}", res);
                        match res {
                            Ok(mut res) => {
                                if !res.content.is_empty() {
                                    msg.add_content(res.content.clone());
                                    yield Ok(StreamedChatResponse::Text(res.content.to_string()));
                                }
                                if !res.think.is_empty() {
                                    msg.add_think(res.think.clone());
                                    yield Ok(StreamedChatResponse::Reasoning(res.think.to_string()));
                                }
                                if let Some(tools) = res.tool_calls {
                                    for tool in tools {
                                        msg.add_tool(tool.clone());
                                        yield Ok(StreamedChatResponse::ToolCall(tool));
                                    }
                                }
                                // 保存token使用情况
                                if let Some(usage) = &res.token_usage {
                                    // 先检查token限制（借用）
                                    if state.check_token_limit(Some(usage)) {
                                        // 超过限制，停止生成
                                        break;
                                    }
                                    // 然后移动值
                                    msg.token_usage = res.token_usage.take();
                                }
                            },
                            Err(e) => yield Err(anyhow::anyhow!(e.to_string())),
                        }
                    }
                }
                yield Ok(StreamedChatResponse::End);
                state.add_message(msg.clone());
                // 处理工具调用
                info!("工具数 {:?}", msg.tool_calls);
                if msg.tool_calls.is_some() && count > 0 {
                    count -= 1;
                    let tool_calls = msg.tool_calls.unwrap();
                    
                    // 检查是否需要询问用户确认
                    if state.should_ask_for_tool_confirmation() {
                        // 设置等待确认状态
                        state.set_waiting_tool_confirmation(true);
                        // 不执行工具调用，等待用户确认
                        break;
                    } else {
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
                            state.add_message(response);
                        }
                    }
                }
                else {
                    break;
                }
            }
            state.unlock();
        }
    }

    /// 处理非流式聊天
    pub fn handle_chat<'a>(
        state: &'a mut ChatState,
        prompt: &'a str,
    ) -> impl Stream<Item = Result<StreamedChatResponse, anyhow::Error>> + 'a {
        state.add_message(ModelMessage::user(prompt.to_string()));
        let cancel_token = state.get_cancel_token();
        state.lock();
        
        stream! {
            let mut count = state.max_tool_try();
            loop {
                let mut msg = ModelMessage::assistant("", "", vec![]);
                {
                    let stream = state.client().chat2(state.context().to_vec());
                    pin_mut!(stream);
                    while let Some(res) = stream.next().await {
                        // 检查是否已取消
                        if cancel_token.is_cancelled() {
                            break;
                        }
                        info!("{:?}", res);
                        match res {
                            Ok(mut res) => {
                                if !res.content.is_empty() {
                                    msg.add_content(res.content.clone());
                                    yield Ok(StreamedChatResponse::Text(res.content.to_string()));
                                }
                                if !res.think.is_empty() {
                                    msg.add_think(res.think.clone());
                                    yield Ok(StreamedChatResponse::Reasoning(res.think.to_string()));
                                }
                                if let Some(tools) = res.tool_calls {
                                    for tool in tools {
                                        msg.add_tool(tool.clone());
                                        yield Ok(StreamedChatResponse::ToolCall(tool));
                                    }
                                }
                                // 保存token使用情况
                                if let Some(usage) = &res.token_usage {
                                    // 先检查token限制（借用）
                                    if state.check_token_limit(Some(usage)) {
                                        // 超过限制，停止生成
                                        break;
                                    }
                                    // 然后移动值
                                    msg.token_usage = res.token_usage.take();
                                }
                            },
                            Err(e) => yield Err(anyhow::anyhow!(e.to_string())),
                        }
                    }
                    yield Ok(StreamedChatResponse::End);
                }
                state.add_message(msg.clone());
                if msg.tool_calls.is_some() && count > 0 {
                    count -= 1;
                    let tool_calls = msg.tool_calls.unwrap();
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
                        state.add_message(response);
                    }
                }
                else {
                    break;
                }
            }
            state.unlock();
        }
    }

    /// 重新聊天（使用已有上下文）
    pub fn handle_rechat(
        state: &mut ChatState,
    ) -> impl Stream<Item = Result<StreamedChatResponse, anyhow::Error>> + '_ {
        let mut tools = Vec::new();
        if let Some(last) = state.context().last() {
            if let Some(calls) = &last.tool_calls {
                for tool in calls {
                    tools.push(tool.clone());
                }
            }
        }
        
        stream! {
            info!("stream_rechat 工具数 {:?}", tools);
            let mut tool_responses = Vec::new();
            {
                let stream = ChatTools::call_tool(tools, state.get_cancel_token());
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
                    if state.get_cancel_token().is_cancelled() {
                        break;
                    }
                }
            }
            for response in tool_responses {
                state.add_message(response);
            }
            let stream = Self::handle_stream_chat(state);
            pin_mut!(stream);
            while let Some(res) = stream.next().await {
                yield res;
            }
        }
    }
}
