use std::{
    cmp::min,
    io::{self},
    sync::{
        atomic::{AtomicBool, Ordering}, mpsc, Arc, Mutex
    },
};

use clap::Parser;
use futures::{StreamExt, pin_mut};
use log::{debug};
use ratatui::{
    crossterm::event::{self, Event, KeyCode, KeyEventKind}, layout::Position, symbols::scrollbar, widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState}, DefaultTerminal, Frame
};

use crate::{
    Args,
    chat::{Chat, StreamedChatResponse},
    mcp,
    model::param::ModelMessage,
    tui::{get_char_width, inputarea::InputArea, messageblock::MessageBlock},
};

/// 终端用户界面应用程序
/// 
/// 管理TUI状态、事件处理和渲染逻辑
pub struct App {
    /// 聊天会话状态，包含模型上下文和工具配置
    chat: Arc<Mutex<Chat>>,
    /// 应用程序退出标志
    should_exit: Arc<AtomicBool>,
    /// 当前垂直滚动位置（行索引）
    index: u16,
    /// 文本输入区域
    input: InputArea,
    /// 窗口高度（不包括输入区域）
    window_height: u16,
    /// 消息块列表，用于渲染
    blocks: Vec<MessageBlock>,
    /// 当前可用宽度（不包括滚动条）
    width: u16,
    /// 所有消息的总行数
    max_line: u16,
    /// 垂直滚动条状态
    vertical_scroll_state: ScrollbarState,
    /// 接收滚动到底部信号的接收器
    scroll_down_rx: mpsc::Receiver<bool>,
    /// 发送滚动到底部信号的发送器
    scroll_down_tx: mpsc::Sender<bool>,
    /// 接收键盘事件的接收器
    event_rx: mpsc::Receiver<Event>,
    /// 发送键盘事件的发送器
    event_tx: mpsc::Sender<Event>,
    /// 脏标志，表示需要重新渲染
    dirty: bool,
    /// 光标在输入区域中的水平偏移（字符宽度）
    cursor_offset: u16,
}

impl App {
    /// 创建新的App实例
    /// 
    /// 根据命令行参数初始化聊天会话，设置事件通道和滚动信号通道
    pub fn new() -> Self {
        let mut chat = Chat::default();
        let args = Args::parse();
        if Some(true) == args.use_tool {
            chat = chat.tools(mcp::get_config_tools());
        }
        let (scroll_tx, scroll_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        Self {
            chat: Arc::new(Mutex::new(chat)),
            should_exit: Arc::new(AtomicBool::new(false)),
            index: 0,
            input: InputArea::default(),
            window_height: 20,
            blocks: vec![],
            width: 20,
            max_line: 100,
            vertical_scroll_state: ScrollbarState::new(1),
            scroll_down_rx: scroll_rx,
            scroll_down_tx: scroll_tx,
            event_rx,
            event_tx,
            dirty: true,
            cursor_offset: 0,
        }
    }

    /// 渲染应用程序界面
    /// 
    /// 将应用程序状态渲染到终端帧中，包括：
    /// - 消息块显示区域
    /// - 垂直滚动条
    /// - 文本输入区域
    /// - 光标位置
    /// 
    /// 此方法根据当前滚动位置和窗口大小计算哪些消息块需要显示，
    /// 并处理部分消息块被截断的情况。
    pub fn render(&mut self, frame: &mut Frame<'_>) {
        let mut area = frame.area();
        // 计算滚动条区域，减去滚动条宽度
        let mut scroll_area = area;
        scroll_area.x = area.width - 1;
        scroll_area.width = 1;
        area.width -= 1;
        self.width = area.width;
        self.refresh();
        // 先计算输入区域
        let mut input_area = area;
        input_area.y = area.height - self.input.height();
        area.height -= self.input.height();
        self.window_height = area.height;
        // 绘制光标
        frame.set_cursor_position(Position::new(
            input_area.x + self.cursor_offset + 1,
            input_area.y + 1,
        ));
        // 处理信息块
        let st = self.index as usize;
        let ed = area.height as usize + st;
        let mut block_start_line = 0;
        let mut height = area.height as usize;
        // 计算在范围内的块、显示块
        for blk in self.blocks.iter() {
            let block_end_line = blk.line_count as usize + block_start_line;
            // 在显示范围内
            if block_end_line > st || block_start_line > ed {
                let mut blk_area = area;
                let blk_height = min(
                    height,
                    min(blk.line_count as usize + block_start_line - st, blk.line_count as usize),
                ) as u16;
                blk_area.height = blk_height;
                debug!(
                    "显示 {:?} {:?} {} {}",
                    area, blk_area, height, blk.line_count
                );
                // 如果前面的文字显示出框，挑后面的显示
                if block_start_line < st {
                    // +3 往后一点，不然显示有问题
                    blk.render_block(
                        blk_area,
                        frame.buffer_mut(),
                        (st - block_start_line) as u16,
                        blk_area.width,
                    );
                } else {
                    frame.render_widget(blk, blk_area);
                }
                height -= blk_area.height as usize;
                area.y = min(blk_area.height + area.y, area.height);
            }
            if height <= 0 {
                break;
            }
            block_start_line += blk.line_count as usize;
        }
        // 渲染滚动条
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalLeft)
                .symbols(scrollbar::VERTICAL)
                .begin_symbol(None)
                .track_symbol(None)
                .end_symbol(None),
            scroll_area,
            &mut self.vertical_scroll_state,
        );
        // 最后渲染输入
        frame.render_widget(&self.input, input_area);
    }

    /// 运行应用程序主循环
    /// 
    /// 启动事件监听线程并处理以下任务：
    /// 1. 监听键盘事件（在后台线程中）
    /// 2. 当界面需要更新时重新渲染
    /// 3. 处理用户输入事件
    /// 4. 响应滚动到底部的信号
    /// 
    /// 循环持续运行直到用户退出（按ESC键）或发生错误。
    pub async fn run(mut self, mut terminal: DefaultTerminal) -> io::Result<()> {
        let t = tokio::spawn(Self::watch_events(
            self.event_tx.clone(),
            self.should_exit.clone(),
        ));
        while !self.should_exit.load(Ordering::Relaxed) {
            if self.dirty {
                terminal.draw(|frame| {
                    self.render(frame);
                })?;
                self.dirty = false;
            }
            self.handle_events()?;
            // 正在运行的话始终拉到最底部
            if self.scroll_down_rx.try_recv().is_ok() {
                self.dirty = true;
                if self.max_line > self.window_height {
                    self.index = self.max_line - self.window_height;
                }
            }
        }
        t.abort();
        Ok(())
    }

    /// 监听键盘事件并将其发送到事件通道
    async fn watch_events(tx: mpsc::Sender<Event>, should_exit: Arc<AtomicBool>) -> io::Result<()> {
        while !should_exit.load(Ordering::Relaxed) {
            match event::read() {
                Ok(Event::Key(key)) => {
                    if let Err(e) = tx.send(Event::Key(key)) {
                        log::error!("Failed to send event: {}", e);
                        break;
                    }
                }
                Ok(_) => {} // 忽略非键盘事件
                Err(e) => {
                    log::error!("Failed to read event: {}", e);
                    break;
                }
            }
        }
        Ok(())
    }

    fn handle_events(&mut self) -> io::Result<()> {
        if let Ok(Event::Key(key)) = self.event_rx.try_recv() {
            self.dirty = true;
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Esc => self.handle_escape_key(),
                    KeyCode::Down | KeyCode::Up | KeyCode::Left | KeyCode::Right => 
                        self.handle_navigation_keys(key.code),
                    KeyCode::Delete | KeyCode::Backspace => self.handle_delete_keys(),
                    KeyCode::Enter => self.handle_enter_key(),
                    KeyCode::Char(c) => self.handle_char_key(c),
                    _ => {}
                }
            }
        }
        Ok(())
    }

    /// 处理ESC键：取消运行或退出应用
    fn handle_escape_key(&mut self) {
        let chat = self.chat.lock().unwrap();
        if chat.is_running() {
            chat.cancel();
        } else {
            self.should_exit.store(true, Ordering::Relaxed);
        }
    }

    /// 处理导航键：上下左右
    fn handle_navigation_keys(&mut self, key_code: KeyCode) {
        match key_code {
            KeyCode::Down => {
                if self.max_line > self.window_height {
                    self.index = min(self.max_line - self.window_height, self.index + 1);
                } else {
                    self.index = 0;
                }
            }
            KeyCode::Up if self.index > 0 => {
                self.index = self.index.saturating_sub(1);
            }
            KeyCode::Left => {
                self.cursor_offset = self
                    .cursor_offset
                    .saturating_sub(self.input.get_previous_char_width(self.cursor_offset));
            }
            KeyCode::Right => {
                let content_width = self.input.get_content_width();
                let next_width = self.cursor_offset + self.input.get_width(self.cursor_offset);
                self.cursor_offset = if next_width > content_width {
                    content_width
                } else {
                    next_width
                };
            }
            _ => {}
        }
    }

    /// 处理删除键：Delete和Backspace
    fn handle_delete_keys(&mut self) {
        if self.cursor_offset > 0 {
            let width = self.input.backspace(self.cursor_offset);
            self.cursor_offset = self.cursor_offset.saturating_sub(width);
        }
    }

    /// 处理回车键：发送消息给模型
    fn handle_enter_key(&mut self) {
        let mut chat = self.chat.lock().unwrap();
        if !chat.is_running() {
            // 检查是否正在等待工具调用确认
            if chat.is_waiting_tool_confirmation() {
                // 处理工具调用确认
                let res = self.input.content.to_lowercase();
                self.input.clear();
                if res == "y" || res == "yes" {
                    chat.confirm_tool_call();
                    // 继续执行工具调用
                    tokio::spawn(Self::handle_tool_execution(
                        self.chat.clone(),
                        self.scroll_down_tx.clone(),
                    ));
                } else if res == "no" || res == "n" {
                    chat.reject_tool_call();
                }
            } else if chat.is_waiting_tool() {
                // 先检查模型是否在等待调用工具，可能存在工具调用次数用尽退出对话的情况
                // yes / y 为继续，n / no 为清除
                let res = self.input.content.to_lowercase();
                self.input.clear();
                if res == "y" || res == "yes" {
                    tokio::spawn(Self::handle_chat(
                        self.chat.clone(),
                        self.input.clone(),
                        self.scroll_down_tx.clone(),
                    ));
                } else if res == "no" || res == "n" {
                    chat.reject_tool_call();
                }
            } else {
                tokio::spawn(Self::handle_chat(
                    self.chat.clone(),
                    self.input.clone(),
                    self.scroll_down_tx.clone(),
                ));
            }
            self.cursor_offset = 0;
            self.input.clear();
        }
    }

    /// 处理字符键：输入文本
    fn handle_char_key(&mut self, c: char) {
        let idx = self.input.get_index_by_width(self.cursor_offset);
        self.cursor_offset += get_char_width(c);
        self.input.add(c, idx as usize);
    }

    /// 刷新应用程序显示状态
    /// 
    /// 根据当前聊天上下文更新消息块列表和滚动条状态：
    /// 1. 从聊天上下文中提取用户和助手消息
    /// 2. 过滤掉系统和工具消息
    /// 3. 为每条消息创建MessageBlock
    /// 4. 计算总行数并更新滚动条状态
    /// 5. 如果工具调用达到上限，显示提示消息
    /// 6. 如果正在等待工具调用确认，显示提示消息
    fn refresh(&mut self) {
        debug!("refresh");
        // 先初始化显示结构
        self.blocks.clear();
        self.max_line = 0;
        
        // 提取需要的信息，然后释放锁
        let (messages, is_waiting_tool, is_waiting_tool_confirmation) = {
            let ctx = self.chat.lock().unwrap();
            let messages: Vec<_> = ctx.context.iter().cloned().collect();
            (messages, ctx.is_waiting_tool() && !ctx.is_running(), ctx.is_waiting_tool_confirmation())
        };
        
        // 添加消息块到显示列表
        for msg in messages {
            // 系统、工具的信息过滤
            if msg.role == "system" || msg.role == "tool" {
                continue;
            }
            // 空信息也过滤
            if msg.content.is_empty() {
                continue;
            }
            self.add_block(MessageBlock::new(msg, self.width));
        }
        
        // 如果工具调用达到上限而中断
        if is_waiting_tool {
            self.add_system_message_block("工具调用次数达到设置上限，是否继续，输入 yes/y 继续，no/n 中断".into());
        }
        
        // 如果正在等待工具调用确认
        if is_waiting_tool_confirmation {
            self.add_system_message_block("检测到工具调用，是否执行？输入 yes/y 执行，no/n 取消".into());
        }
        
        self.update_scrollbar_state();
    }

    /// 添加系统消息块
    fn add_system_message_block(&mut self, content: String) {
        self.add_block(MessageBlock::new(ModelMessage::system(content), self.width));
    }

    /// 添加消息块并更新总行数
    fn add_block(&mut self, block: MessageBlock) {
        self.max_line += block.line_count;
        self.blocks.push(block);
    }

    /// 更新滚动条状态
    fn update_scrollbar_state(&mut self) {
        if self.max_line > self.window_height {
            self.vertical_scroll_state = self
                .vertical_scroll_state
                .content_length((self.max_line - self.window_height) as usize);
        } else {
            self.vertical_scroll_state = self.vertical_scroll_state.content_length(1);
        }
        self.vertical_scroll_state = self.vertical_scroll_state.position(self.index as usize);
    }

    /// 处理与模型的聊天交互
    /// 
    /// 此异步方法执行以下操作：
    /// 1. 将用户输入添加到聊天上下文
    /// 2. 启动流式聊天响应
    /// 3. 处理流式响应（文本、工具调用、推理等）
    /// 4. 发送滚动到底部的信号
    /// 5. 更新聊天上下文以包含完整的响应和token使用信息
    async fn handle_chat(selfchat: Arc<Mutex<Chat>>, input: InputArea, tx: mpsc::Sender<bool>) {
        // 获取聊天实例并克隆
        let mut chat = {
            let guard = selfchat.lock().unwrap();
            guard.clone()
        };
        
        // 添加用户输入到聊天上下文
        Self::add_user_input_to_context(&selfchat, &input, &mut chat);
        
        // 锁定聊天状态并启动流式响应
        selfchat.lock().unwrap().lock();
        let stream = chat.stream_rechat();
        
        // 发送初始滚动信号
        Self::send_scroll_signal(&tx);
        
        // 处理流式响应
        Self::process_stream_responses(&selfchat, stream, &tx).await;
        
        // 更新聊天上下文并解锁
        {
            let mut guard = selfchat.lock().unwrap();
            guard.context = chat.context;
            guard.unlock();
        }
    }

    /// 添加用户输入到聊天上下文
    fn add_user_input_to_context(selfchat: &Arc<Mutex<Chat>>, input: &InputArea, chat: &mut Chat) {
        if !input.content.is_empty() {
            chat.context.push(ModelMessage::user(input.content.clone()));
            selfchat
                .lock()
                .unwrap()
                .context
                .push(ModelMessage::user(input.content.clone()));
        }
    }

    /// 发送滚动到底部信号
    fn send_scroll_signal(tx: &mpsc::Sender<bool>) {
        if let Err(e) = tx.send(true) {
            log::error!("Failed to send scroll signal: {}", e);
        }
    }

    /// 处理流式响应错误
    fn handle_stream_error(selfchat: &Arc<Mutex<Chat>>, err: impl std::fmt::Display) {
        log::error!("Stream response error: {}", err);
        let mut ctx = selfchat.lock().unwrap();
        ctx.context.push(ModelMessage::assistant(err.to_string(), "".into(), vec![]));
    }

    /// 确保上下文中存在一个assistant消息，如果不存在则创建一个
    fn ensure_assistant_message(ctx: &mut std::sync::MutexGuard<'_, Chat>) -> usize {
        let last_is_assistant = ctx.context.last()
            .map(|m| m.role == "assistant")
            .unwrap_or(false);
        
        if !last_is_assistant {
            ctx.context.push(ModelMessage::assistant("".into(), "".into(), vec![]));
        }
        ctx.context.len() - 1
    }

    /// 处理流式响应
    async fn handle_stream_response(selfchat: &Arc<Mutex<Chat>>, response: StreamedChatResponse) {
        match response {
            StreamedChatResponse::Text(text) => {
                let mut ctx = selfchat.lock().unwrap();
                let idx = Self::ensure_assistant_message(&mut ctx);
                ctx.context[idx].add_content(text);
            }
            StreamedChatResponse::ToolCall(tool_call) => {
                let mut ctx = selfchat.lock().unwrap();
                let idx = Self::ensure_assistant_message(&mut ctx);
                ctx.context[idx].add_tool(tool_call);
            }
            StreamedChatResponse::Reasoning(think) => {
                let mut ctx = selfchat.lock().unwrap();
                let idx = Self::ensure_assistant_message(&mut ctx);
                ctx.context[idx].add_think(think);
            }
            StreamedChatResponse::ToolResponse(tool) => {
                let mut ctx = selfchat.lock().unwrap();
                ctx.context.push(tool);
            }
            StreamedChatResponse::End => {
                // End事件表示模型响应完成，此时chat.context中应该已经包含了完整的消息
                // 包括token_usage信息
            }
        }
    }

    /// 处理流式响应循环
    async fn process_stream_responses(
        selfchat: &Arc<Mutex<Chat>>,
        stream: impl futures::Stream<Item = Result<StreamedChatResponse, impl std::fmt::Display>>,
        tx: &mpsc::Sender<bool>,
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
                    Self::handle_stream_error(selfchat, err);
                    break;
                }
                None => {
                    break;
                }
            }
        }
    }

    /// 处理工具执行
    async fn handle_tool_execution(selfchat: Arc<Mutex<Chat>>, tx: mpsc::Sender<bool>) {
        // 获取聊天实例并克隆
        let chat = {
            let guard = selfchat.lock().unwrap();
            guard.clone()
        };
        
        // 锁定聊天状态
        selfchat.lock().unwrap().lock();
        
        // 获取最后一个消息中的工具调用
        let tool_calls = {
            let guard = selfchat.lock().unwrap();
            if let Some(last) = guard.context.last() {
                if let Some(tools) = &last.tool_calls {
                    tools.clone()
                } else {
                    vec![]
                }
            } else {
                vec![]
            }
        };
        
        if !tool_calls.is_empty() {
            // 执行工具调用
            let stream = chat.call_tool(tool_calls);
            pin_mut!(stream);
            
            // 发送滚动信号
            Self::send_scroll_signal(&tx);
            
            // 处理工具响应
            while let Some(res) = stream.next().await {
                match res {
                    Ok(tool_response) => {
                        // 添加工具响应到上下文
                        let mut guard = selfchat.lock().unwrap();
                        guard.context.push(tool_response);
                        if guard.context.len() > guard.max_context_num {
                            guard.context.remove(0);
                        }
                        // 发送滚动信号
                        Self::send_scroll_signal(&tx);
                    }
                    Err(e) => {
                        log::error!("工具调用错误: {}", e);
                        break;
                    }
                }
            }
        }
        
        // 解锁聊天状态
        selfchat.lock().unwrap().unlock();
    }
}
