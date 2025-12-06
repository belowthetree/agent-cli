use std::{
    io,
    sync::{atomic::AtomicBool, mpsc, Arc},
};

use log::error;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};

use crate::tui::app::App;

/// 事件处理器，负责处理键盘事件和事件监听
pub struct AppEvent;

impl AppEvent {
    /// 监听键盘事件并将其发送到事件通道
    pub async fn watch_events(
        tx: mpsc::Sender<Event>,
        should_exit: Arc<AtomicBool>,
    ) -> io::Result<()> {
        while !should_exit.load(std::sync::atomic::Ordering::Relaxed) {
            match event::read() {
                Ok(Event::Key(key)) => {
                    if let Err(e) = tx.send(Event::Key(key)) {
                        error!("Failed to send event: {}", e);
                        break;
                    }
                }
                Ok(_) => {} // 忽略非键盘事件
                Err(e) => {
                    error!("Failed to read event: {}", e);
                    break;
                }
            }
        }
        Ok(())
    }

    /// 处理ESC键：取消运行或退出应用
    pub fn handle_escape_key(app: &mut App) {
        let chat = app.chat.lock().unwrap();
        if chat.is_running() {
            chat.cancel();
        } else {
            app.should_exit.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// 处理导航键：上下左右
    pub fn handle_navigation_keys(app: &mut App, key_code: KeyCode) {
        match key_code {
            KeyCode::Down => {
                if app.max_line > app.window_height {
                    app.index = std::cmp::min(app.max_line - app.window_height, app.index + 1);
                } else {
                    app.index = 0;
                }
            }
            KeyCode::Up if app.index > 0 => {
                app.index = app.index.saturating_sub(1);
            }
            KeyCode::Left => {
                app.cursor_offset = app
                    .cursor_offset
                    .saturating_sub(app.input.get_previous_char_width(app.cursor_offset));
            }
            KeyCode::Right => {
                let content_width = app.input.get_content_width();
                let next_width = app.cursor_offset + app.input.get_width(app.cursor_offset);
                app.cursor_offset = if next_width > content_width {
                    content_width
                } else {
                    next_width
                };
            }
            _ => {}
        }
    }

    /// 处理删除键：Delete和Backspace
    pub fn handle_delete_keys(app: &mut App) {
        if app.cursor_offset > 0 {
            let width = app.input.backspace(app.cursor_offset);
            app.cursor_offset = app.cursor_offset.saturating_sub(width);
        }
    }

    /// 处理回车键：发送消息给模型
    pub fn handle_enter_key(app: &mut App) {
        let mut chat = app.chat.lock().unwrap();
        if !chat.is_running() {
            // 检查是否正在等待工具调用确认
            if chat.is_waiting_tool_confirmation() {
                // 处理工具调用确认
                let res = app.input.content.to_lowercase();
                app.input.clear();
                if res == "y" || res == "yes" {
                    chat.confirm_tool_call();
                    // 继续执行工具调用
                    tokio::spawn(crate::tui::appchat::AppChat::handle_tool_execution(
                        app.chat.clone(),
                        app.scroll_down_tx.clone(),
                    ));
                } else if res == "no" || res == "n" {
                    chat.reject_tool_call();
                }
            } else if chat.is_waiting_tool() {
                // 先检查模型是否在等待调用工具，可能存在工具调用次数用尽退出对话的情况
                // yes / y 为继续，n / no 为清除
                let res = app.input.content.to_lowercase();
                app.input.clear();
                if res == "y" || res == "yes" {
                    tokio::spawn(crate::tui::appchat::AppChat::handle_chat(
                        app.chat.clone(),
                        app.input.clone(),
                        app.scroll_down_tx.clone(),
                    ));
                } else if res == "no" || res == "n" {
                    chat.reject_tool_call();
                }
            } else {
                tokio::spawn(crate::tui::appchat::AppChat::handle_chat(
                    app.chat.clone(),
                    app.input.clone(),
                    app.scroll_down_tx.clone(),
                ));
            }
            app.cursor_offset = 0;
            app.input.clear();
        }
    }

    /// 处理字符键：输入文本
    pub fn handle_char_key(app: &mut App, c: char) {
        let idx = app.input.get_index_by_width(app.cursor_offset);
        app.cursor_offset += crate::tui::get_char_width(c);
        app.input.add(c, idx as usize);
    }

    /// 处理所有事件
    pub fn handle_events(app: &mut App) -> io::Result<()> {
        if let Ok(Event::Key(key)) = app.event_rx.try_recv() {
            app.dirty = true;
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Esc => Self::handle_escape_key(app),
                    KeyCode::Down | KeyCode::Up | KeyCode::Left | KeyCode::Right => {
                        Self::handle_navigation_keys(app, key.code)
                    }
                    KeyCode::Delete | KeyCode::Backspace => Self::handle_delete_keys(app),
                    KeyCode::Enter => Self::handle_enter_key(app),
                    KeyCode::Char(c) => Self::handle_char_key(app, c),
                    _ => {}
                }
            }
        }
        Ok(())
    }
}
