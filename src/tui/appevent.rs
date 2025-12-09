use std::{
    io,
    sync::{atomic::AtomicBool, mpsc, Arc},
};

use log::{error, info};
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
        // 如果选项对话框可见，优先取消选项对话框
        if app.option_dialog.visible {
            app.option_dialog.hide();
            return;
        }
        
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
                // 如果显示选项对话框，则选择下一个选项
                if app.option_dialog.visible {
                    app.option_dialog.next();
                }
                // 否则如果显示命令提示，则选择下一个命令提示
                else if app.input.should_show_suggestions() {
                    app.input.next_suggestion();
                } else {
                    if app.max_line > app.window_height {
                        app.index = std::cmp::min(app.max_line - app.window_height, app.index + 1);
                    } else {
                        app.index = 0;
                    }
                }
            }
            KeyCode::Up => {
                // 如果显示选项对话框，则选择上一个选项
                if app.option_dialog.visible {
                    app.option_dialog.previous();
                }
                // 否则如果显示命令提示，则选择上一个命令提示
                else if app.input.should_show_suggestions() {
                    app.input.previous_suggestion();
                } else if app.index > 0 {
                    app.index = app.index.saturating_sub(1);
                }
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
            // 检查是否需要更新命令提示
            app.check_command_suggestions();
        }
    }

    /// 处理回车键：发送消息给模型或执行命令
    pub fn handle_enter_key(app: &mut App) {
        // 首先检查是否显示选项对话框
        if app.option_dialog.visible {
            // 先获取需要的数据，避免同时借用
            let selected_option = app.option_dialog.get_selected_option().cloned();
            let selected_index = app.option_dialog.get_selected_index().unwrap_or(0);
            let _title = app.option_dialog.title.clone();
            
            // 隐藏选项对话框
            app.option_dialog.hide();
            
            // 如果有选中的选项，显示系统消息
            if let Some(selected_option) = selected_option {
                // 添加系统消息显示用户的选择
                app.add_system_message(&format!(
                    "已选择: {} (选项 {})",
                    selected_option,
                    selected_index + 1
                ));
                
                // 这里可以添加回调机制来处理选项选择
                // 例如：app.handle_option_selection(&title, selected_index, &selected_option);
            }
            return;
        }
        
        // 然后检查是否显示命令提示
        if app.input.should_show_suggestions() {
            // 获取选中的命令并克隆它，以释放对app.input的借用
            let command = app.input.get_selected_command().cloned();
            if let Some(command) = command {
                // 清空输入并隐藏命令提示
                app.input.clear();
                app.input.hide_suggestions();
                app.cursor_offset = 0;
                
                // 使用block_in_place来执行阻塞操作
                // 这会通知Tokio运行时当前线程将暂时阻塞
                tokio::task::block_in_place(|| {
                    // 在block_in_place中创建新的运行时
                    // 这会在当前线程中创建一个新的运行时，而不是在Tokio工作线程中
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    rt.block_on(async {
                        app.execute_command(&command).await;
                    });
                });
                return;
            }
        }
        
        let mut chat = app.chat.lock().unwrap();
        if !chat.is_running() {
            // 检查是否正在等待对话轮次确认
            if chat.is_waiting_context_confirmation() {
                // 处理对话轮次确认
                let res = app.input.content.to_lowercase();
                app.input.clear();
                if res == "y" || res == "yes" {
                    info!("重置对话");
                    // 用户选择继续，重置对话轮次计数
                    chat.reset_conversation_turn();
                    chat.set_waiting_context_confirmation(false);
                    // 继续处理当前输入
                    let input_clone = app.input.clone();
                    let chat_clone = app.chat.clone();
                    let scroll_tx_clone = app.scroll_down_tx.clone();
                    tokio::spawn(async move {
                        crate::tui::appchat::AppChat::handle_chat(
                            chat_clone,
                            input_clone,
                            scroll_tx_clone,
                        ).await;
                    });
                } else if res == "no" || res == "n" {
                    // 用户选择停止，清除等待状态但不重置计数
                    chat.set_waiting_context_confirmation(false);
                }
            }
            // 检查是否正在等待工具调用确认
            else if chat.is_waiting_tool_confirmation() {
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
        // 检查是否需要更新命令提示
        app.check_command_suggestions();
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
