use std::{
    io::{self},
    sync::{
        atomic::{AtomicBool, Ordering}, mpsc, Arc, Mutex
    },
};

use clap::Parser;
use ratatui::{
    crossterm::event::{Event}, widgets::ScrollbarState, DefaultTerminal, Frame
};

use crate::{
    Args, chat::Chat, mcp, tui::{
        appevent::AppEvent,
        inputarea::InputArea,
        messageblock::MessageBlock,
        renderer::Renderer,
        state_manager::StateManager,
    }
};

/// 终端用户界面应用程序
/// 
/// 管理TUI状态、事件处理和渲染逻辑
pub struct App {
    /// 聊天会话状态，包含模型上下文和工具配置
    pub chat: Arc<Mutex<Chat>>,
    /// 应用程序退出标志
    pub should_exit: Arc<AtomicBool>,
    /// 当前垂直滚动位置（行索引）
    pub index: u16,
    /// 文本输入区域
    pub input: InputArea,
    /// 窗口高度（不包括输入区域）
    pub window_height: u16,
    /// 消息块列表，用于渲染
    pub blocks: Vec<MessageBlock>,
    /// 当前可用宽度（不包括滚动条）
    pub width: u16,
    /// 所有消息的总行数
    pub max_line: u16,
    /// 垂直滚动条状态
    pub vertical_scroll_state: ScrollbarState,
    /// 接收滚动到底部信号的接收器
    pub scroll_down_rx: mpsc::Receiver<bool>,
    /// 发送滚动到底部信号的发送器
    pub scroll_down_tx: mpsc::Sender<bool>,
    /// 接收键盘事件的接收器
    pub event_rx: mpsc::Receiver<Event>,
    /// 发送键盘事件的发送器
    pub event_tx: mpsc::Sender<Event>,
    /// 脏标志，表示需要重新渲染
    pub dirty: bool,
    /// 光标在输入区域中的水平偏移（字符宽度）
    pub cursor_offset: u16,
    /// 可用命令列表
    pub commands: Vec<String>,
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
        
        // 初始化命令注册器并获取命令列表
        let registry = crate::tui::init_global_registry();
        let commands = registry.command_names();
        
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
            commands,
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
        Renderer::render(self, frame);
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
        let t = tokio::spawn(AppEvent::watch_events(
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
            AppEvent::handle_events(&mut self)?;
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


    /// 刷新应用程序显示状态
    /// 
    /// 根据当前聊天上下文更新消息块列表和滚动条状态：
    /// 1. 从聊天上下文中提取用户和助手消息
    /// 2. 过滤掉系统和工具消息
    /// 3. 为每条消息创建MessageBlock
    /// 4. 计算总行数并更新滚动条状态
    /// 5. 如果工具调用达到上限，显示提示消息
    /// 6. 如果正在等待工具调用确认，显示提示消息
    pub fn refresh(&mut self) {
        StateManager::refresh(self);
    }

    /// 检查并更新命令提示
    pub fn check_command_suggestions(&mut self) {
        let content = self.input.content.clone();
        if content.starts_with('/') {
            self.input.update_suggestions(&self.commands, &content);
        } else {
            self.input.hide_suggestions();
        }
    }

    /// 执行命令
    pub async fn execute_command(&mut self, command: &str) -> bool {
        // 移除开头的斜杠
        let command = command.trim_start_matches('/');
        
        // 分割命令和参数
        let parts: Vec<&str> = command.splitn(2, ' ').collect();
        let cmd_name = parts[0];
        let args = if parts.len() > 1 { parts[1] } else { "" };
        
        // 从注册器中查找命令
        let registry = crate::tui::global_registry();
        if let Some(cmd) = registry.find(cmd_name) {
            // 执行命令
            cmd.execute(self, args).await
        } else {
            false
        }
    }

    /// 添加系统消息
    pub fn add_system_message(&mut self, message: &str) {
        use crate::model::param::ModelMessage;
        let model_message = ModelMessage::system(message.to_string());
        let block = MessageBlock::new(model_message, self.width);
        self.blocks.push(block);
        self.refresh();
    }
}
