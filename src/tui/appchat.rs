use std::sync::{mpsc, Arc, Mutex};

use futures::{pin_mut, StreamExt};
use log::{error, info};

use crate::{
    chat::{Chat, EChatState, StreamedChatResponse},
    model::param::ModelMessage,
    tui::{app::ETuiEvent, send_event, ui::inputarea::InputArea},
};

/// 聊天处理器，负责处理聊天和工具执行逻辑
pub struct AppChat;

impl AppChat {
    /// 处理与模型的聊天交互
    /// 
    /// 此异步方法执行以下操作：
    /// 1. 将用户输入添加到聊天上下文
    /// 2. 启动流式聊天响应
    /// 3. 处理流式响应（文本、工具调用、推理等）
    /// 4. 发送滚动到底部的信号
    /// 5. 更新聊天上下文以包含完整的响应和token使用信息
    pub async fn handle_chat(
        mut idx: usize,
        selfchat: Arc<Mutex<Chat>>,
        input: InputArea,
        tx: mpsc::Sender<ETuiEvent>,
    ) {
        info!("处理聊天");
        {
            if selfchat.lock().unwrap().get_state() != EChatState::Idle {
                info!("正忙碌");
                return;
            }
        }
        // 获取聊天实例并克隆
        if !input.content.is_empty() {
            selfchat.lock().unwrap().add_message(ModelMessage::user(input.content.clone()));
            selfchat
                .lock()
                .unwrap()
                .add_message(ModelMessage::user(input.content.clone()));
            send_event(&tx, ETuiEvent::AddMessage(ModelMessage::user(input.content.clone())));
            idx += 1;
        }

        let mut chat = { selfchat.lock().unwrap().clone() };
        {
            selfchat.lock().unwrap().run();
        }
        let stream = chat.stream_rechat();
        // 发送初始滚动信号
        send_event(&tx, ETuiEvent::ScrollToBottom);
        // 处理流式响应
        Self::process_stream_responses(idx, stream, &tx).await;

        // 更新聊天上下文
        *selfchat.lock().unwrap() = chat;
    }

    /// 处理流式响应错误
    fn handle_stream_error(err: impl std::fmt::Display, tx: &mpsc::Sender<ETuiEvent>) {
        error!("Stream response error: {}", err);
        let msg = ModelMessage::info(err.to_string());
        send_event(&tx, ETuiEvent::AddMessage(msg));
    }

    /// 处理流式响应循环
    /// 传入 idx 为当前消息的插入位置
    async fn process_stream_responses(
        mut idx: usize,
        stream: impl futures::Stream<Item = Result<StreamedChatResponse, impl std::fmt::Display>>,
        tx: &mpsc::Sender<ETuiEvent>,
    ) {
        pin_mut!(stream);
        
        loop {
            // 发送滚动信号以确保界面更新
            send_event(&tx, ETuiEvent::ScrollToBottom);
            match stream.next().await {
                Some(Ok(response)) => {
                    match response {
                        StreamedChatResponse::Text(text) => {
                            if let Err(e) = tx.send(ETuiEvent::UpdateMessage(idx, ModelMessage::assistant(text, "", vec![]))) {
                                error!("{:?}", e);
                            }
                        }
                        StreamedChatResponse::ToolCall(tool_call) => {
                            if let Err(e) = tx.send(ETuiEvent::UpdateMessage(idx, ModelMessage::assistant("", "", vec![tool_call]))) {
                                error!("{:?}", e);
                            }
                        }
                        StreamedChatResponse::Reasoning(think) => {
                            if let Err(e) = tx.send(ETuiEvent::UpdateMessage(idx, ModelMessage::assistant("", think, vec![]))) {
                                error!("{:?}", e);
                            }
                        }
                        StreamedChatResponse::ToolResponse(tool) => {
                            if let Err(e) = tx.send(ETuiEvent::UpdateMessage(idx, tool)) {
                                error!("{:?}", e);
                            }
                        }
                        StreamedChatResponse::TokenUsage(usage) => {
                            if let Err(e) = tx.send(ETuiEvent::UpdateMessage(idx, ModelMessage::token(usage))) {
                                error!("{:?}", e);
                            }
                        }
                        StreamedChatResponse::End => {
                            idx += 1;
                        }
                    }
                }
                Some(Err(err)) => {
                    Self::handle_stream_error(err, tx);
                    break;
                }
                None => {
                    break;
                }
            }
        }
    }

    /// 处理工具执行
    pub async fn handle_tool_execution(
        idx: usize,
        selfchat: Arc<Mutex<Chat>>,
        tx: mpsc::Sender<ETuiEvent>,
    ) {
        // 检查是否正在等待对话轮次确认
        let mut guard = {selfchat.lock().unwrap().clone()};
        if guard.get_state() != EChatState::Idle {
            info!("正忙碌");
            return;
        }
        {
            selfchat.lock().unwrap().run();
        }
        let stream = guard.stream_rechat();
        send_event(&tx, ETuiEvent::ScrollToBottom);
        // 处理流式响应
        Self::process_stream_responses(idx, stream, &tx).await;
        *selfchat.lock().unwrap() = guard;
    }
}
