use std::{
    io::{self},
    sync::{
        mpsc, Arc, Mutex
    },
};

use clap::Parser;
use ratatui::{
    crossterm::event::KeyEvent, widgets::ScrollbarState, DefaultTerminal, Frame
};

use crate::{
    Args, chat::Chat, mcp, model::param::ModelMessage, tui::{
        appevent::AppEvent, option_dialog::OptionDialog, renderer::Renderer, state_manager::StateManager, ui::{inputarea::InputArea, messageblock::MessageBlock}
    }
};

#[derive(PartialEq)]
pub enum ETuiEvent {
    KeyEvent(KeyEvent),
    InfoMessage(usize, ModelMessage),
    ScrollToBottom,
    RefreshUI,
    Exit,
}

/// 终端用户界面应用程序
/// 
/// 管理TUI状态、事件处理和渲染逻辑
pub struct App {
    /// 聊天会话状态，包含模型上下文和工具配置
    pub chat: Arc<Mutex<Chat>>,
    /// 当前垂直滚动位置（行索引）
    pub index: u16,
    /// 文本输入区域
    pub input: InputArea,
    /// 窗口高度（不包括输入区域）
    pub window_height: u16,
    /// 消息块列表，用于渲染
    pub blocks: Vec<MessageBlock>,
    /// 信息消息列表，用于显示指令、提示等信息
    /// 每个元素是一个元组：(插入位置, 消息)
    /// 插入位置表示该信息消息应该插入到聊天上下文中的哪个位置
    pub info_messages: Vec<(usize, ModelMessage)>,
    /// 当前可用宽度（不包括滚动条）
    pub width: u16,
    /// 所有消息的总行数
    pub max_line: u16,
    /// 垂直滚动条状态
    pub vertical_scroll_state: ScrollbarState,
    /// 接收TUI事件的接收器
    event_rx: mpsc::Receiver<ETuiEvent>,
    /// 发送TUI事件的发送器
    pub event_tx: mpsc::Sender<ETuiEvent>,
    /// 光标在输入区域中的水平偏移（字符宽度）
    pub cursor_offset: u16,
    /// 可用命令列表
    pub commands: Vec<String>,
    /// 选项对话框
    pub option_dialog: OptionDialog,
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
        let (event_tx, event_rx) = mpsc::channel::<ETuiEvent>();
        
        // 初始化命令注册器并获取命令列表
        let registry = crate::tui::init_global_registry();
        let commands = registry.command_names();
        
        Self {
            chat: Arc::new(Mutex::new(chat)),
            index: 0,
            input: InputArea::default(),
            window_height: 20,
            blocks: vec![],
            info_messages: vec![],
            width: 20,
            max_line: 100,
            vertical_scroll_state: ScrollbarState::new(1),
            event_rx,
            event_tx,
            cursor_offset: 0,
            commands,
            option_dialog: OptionDialog::new(),
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
        terminal.draw(|frame| {
            self.render(frame);
        })?;
        loop {
            if let Ok(ev) = self.event_rx.try_recv() {
                if ev == ETuiEvent::Exit {
                    break;
                } else if ev == ETuiEvent::RefreshUI {
                    terminal.draw(|frame| {
                        self.render(frame);
                    })?;
                }
                AppEvent::watch_events(
                    self.event_tx.clone(),
                )?;
                AppEvent::handle_events(&mut self, ev)?;
            }
        }
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
        let mut model_message = ModelMessage::system(message.to_string());
        model_message.role = "info".into(); // 使用特殊角色
        
        // 获取当前聊天上下文中的消息数量，作为插入位置
        let insert_position = {
            let chat = self.chat.lock().unwrap();
            chat.context().len()
        };
        
        // 通过事件通道发送信息消息
        if let Err(e) = self.event_tx.send(ETuiEvent::InfoMessage(insert_position, model_message)) {
            log::error!("Failed to send info message event: {}", e);
        }
    }
    
    /// 添加信息消息
    /// 
    /// 添加一个信息消息到信息消息列表中，用于显示指令、提示等信息。
    /// 这些消息会显示在聊天消息之间。
    pub fn add_info_message(&mut self, message: &str) {
        use crate::model::param::ModelMessage;
        let mut model_message = ModelMessage::system(message.to_string());
        model_message.role = "info".into(); // 使用特殊角色
        
        // 获取当前聊天上下文中的消息数量，作为插入位置
        let insert_position = {
            let chat = self.chat.lock().unwrap();
            chat.context().len()
        };
        
        // 通过事件通道发送信息消息
        if let Err(e) = self.event_tx.send(ETuiEvent::InfoMessage(insert_position, model_message)) {
            log::error!("Failed to send info message event: {}", e);
        }
    }
    
    /// 显示选项对话框
    /// 
    /// 显示一个选项对话框供用户选择，用户可以使用上下键导航，回车键确认，ESC键取消。
    /// 
    /// # 参数
    /// - `title`: 对话框标题
    /// - `options`: 选项列表
    #[allow(dead_code)]
    pub fn show_option_dialog(&mut self, title: &str, options: Vec<String>) {
        self.option_dialog.show(title, options);
    }
}
