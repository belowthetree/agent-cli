use std::sync::{mpsc, Arc, Mutex};

use futures::{pin_mut, StreamExt};
use log::{error, info};

use crate::{
    chat::{Chat, EChatState, StreamedChatResponse},
    model::param::ModelMessage,
    tui::{app::ETuiEvent, ui::inputarea::InputArea},
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
        selfchat: Arc<Mutex<Chat>>,
        input: InputArea,
        tx: mpsc::Sender<ETuiEvent>,
    ) {
        // 检查是否正在等待对话轮次确认
        {
            let guard = selfchat.lock().unwrap();
            if guard.get_state() != EChatState::Idle {
                // 如果正在等待确认，不处理新的聊天
                return;
            }
        }
        
        // 获取聊天实例并克隆
        let mut chat = {
            let guard = selfchat.lock().unwrap();
            guard.clone()
        };
        
        // 添加用户输入到聊天上下文
        Self::add_user_input_to_context(&selfchat, &input, &mut chat);

        let stream = chat.stream_rechat();
        
        // 发送初始滚动信号
        Self::send_scroll_signal(&tx);
        
        // 处理流式响应
        Self::process_stream_responses(&selfchat, stream, &tx).await;
        
        // 更新聊天上下文并解锁
        {
            let mut guard = selfchat.lock().unwrap();
            // 清空当前上下文，然后添加所有消息
            guard.clear_context();
            for message in chat.context() {
                guard.add_message(message.clone());
            }
        }
    }

    /// 添加用户输入到聊天上下文
    fn add_user_input_to_context(
        selfchat: &Arc<Mutex<Chat>>,
        input: &InputArea,
        chat: &mut Chat,
    ) {
        if !input.content.is_empty() {
            chat.add_message(ModelMessage::user(input.content.clone()));
            selfchat
                .lock()
                .unwrap()
                .add_message(ModelMessage::user(input.content.clone()));
        }
    }

    /// 发送滚动到底部信号
    fn send_scroll_signal(tx: &mpsc::Sender<ETuiEvent>) {
        if let Err(e) = tx.send(ETuiEvent::ScrollToBottom) {
            error!("Failed to send scroll signal: {}", e);
        }
    }

    /// 处理流式响应错误
    fn handle_stream_error(selfchat: &Arc<Mutex<Chat>>, err: impl std::fmt::Display, tx: &mpsc::Sender<ETuiEvent>) {
        error!("Stream response error: {}", err);
        let insert_position = {
            let chat = selfchat.lock().unwrap();
            chat.context().len()
        };
        let mut msg = ModelMessage::system(err.to_string());
        msg.role = "info".into(); // 使用特殊角色
        if let Err(e) = tx.send(ETuiEvent::InfoMessage(insert_position, msg)) {
            log::error!("Failed to send info message event: {}", e);
        }
    }

    /// 确保上下文中存在一个assistant消息，如果不存在则创建一个
    fn ensure_assistant_message(ctx: &mut std::sync::MutexGuard<'_, Chat>) -> usize {
        let last_is_assistant = ctx.context().last()
            .map(|m| m.role == "assistant")
            .unwrap_or(false);
        
        if !last_is_assistant {
            ctx.add_message(ModelMessage::assistant("", "", vec![]));
        }
        ctx.context().len() - 1
    }

    /// 处理流式响应
    async fn handle_stream_response(selfchat: &Arc<Mutex<Chat>>, response: StreamedChatResponse) {
        match response {
            StreamedChatResponse::Text(text) => {
                let mut ctx = selfchat.lock().unwrap();
                let _idx = Self::ensure_assistant_message(&mut ctx);
                // 使用 context_mut() 获取可变引用
                if let Some(last) = ctx.context_mut().last_mut() {
                    last.add_content(text);
                }
            }
            StreamedChatResponse::ToolCall(tool_call) => {
                let mut ctx = selfchat.lock().unwrap();
                let _idx = Self::ensure_assistant_message(&mut ctx);
                if let Some(last) = ctx.context_mut().last_mut() {
                    last.add_tool(tool_call);
                }
            }
            StreamedChatResponse::Reasoning(think) => {
                let mut ctx = selfchat.lock().unwrap();
                let _idx = Self::ensure_assistant_message(&mut ctx);
                if let Some(last) = ctx.context_mut().last_mut() {
                    last.add_think(think);
                }
            }
            StreamedChatResponse::ToolResponse(tool) => {
                let mut ctx = selfchat.lock().unwrap();
                ctx.add_message(tool);
            }
            StreamedChatResponse::End => {
                // End事件表示模型响应完成，此时chat.context()中应该已经包含了完整的消息
                // 包括token_usage信息
                
                // 增加对话轮次计数（模型每次回复时增加1）
                let mut ctx = selfchat.lock().unwrap();
                ctx.increment_conversation_turn();
                // 检查是否超过最大对话轮次
                info!("对话轮次 {:?}", ctx.get_conversation_turn_info());
            }
        }
    }

    /// 处理流式响应循环
    async fn process_stream_responses(
        selfchat: &Arc<Mutex<Chat>>,
        stream: impl futures::Stream<Item = Result<StreamedChatResponse, impl std::fmt::Display>>,
        tx: &mpsc::Sender<ETuiEvent>,
    ) {
        pin_mut!(stream);
        
        loop {
            // 发送滚动信号以确保界面更新
            Self::send_scroll_signal(tx);
            
            match stream.next().await {
                Some(Ok(response)) => {
                    Self::handle_stream_response(selfchat, response).await;
                }
                Some(Err(err)) => {
                    Self::handle_stream_error(selfchat, err, tx);
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
        selfchat: Arc<Mutex<Chat>>,
        tx: mpsc::Sender<ETuiEvent>,
    ) {
        // 获取聊天实例并克隆
        let chat = {
            let guard = selfchat.lock().unwrap();
            guard.clone()
        };

        // 执行工具调用
        let stream = chat.call_tool();
        pin_mut!(stream);
        
        // 发送滚动信号
        Self::send_scroll_signal(&tx);
        
        // 处理工具响应
        while let Some(res) = stream.next().await {
            match res {
                Ok(tool_response) => {
                    // 添加工具响应到上下文
                    let mut guard = selfchat.lock().unwrap();
                    guard.add_message(tool_response);
                    // 发送滚动信号
                    Self::send_scroll_signal(&tx);
                }
                Err(e) => {
                    error!("工具调用错误: {}", e);
                    break;
                }
            }
        }
    }
}
