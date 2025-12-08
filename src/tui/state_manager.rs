use log::debug;

use crate::{
    model::param::ModelMessage,
    tui::{app::App, messageblock::MessageBlock},
};

/// 状态管理器，负责管理应用程序状态
pub struct StateManager;

impl StateManager {
    /// 刷新应用程序显示状态
    /// 
    /// 根据当前聊天上下文更新消息块列表和滚动条状态：
    /// 1. 从聊天上下文中提取用户和助手消息
    /// 2. 过滤掉系统和工具消息
    /// 3. 合并信息消息（按照插入位置插入到聊天消息之间）
    /// 4. 为每条消息创建MessageBlock
    /// 5. 计算总行数并更新滚动条状态
    /// 6. 如果工具调用达到上限，显示提示消息
    /// 7. 如果正在等待工具调用确认，显示提示消息
    /// 8. 如果正在等待对话轮次确认，显示提示消息
    pub fn refresh(app: &mut App) {
        debug!("refresh");
        // 先初始化显示结构
        app.blocks.clear();
        app.max_line = 0;
        
        // 提取需要的信息，然后释放锁
        let (messages, is_waiting_tool, is_waiting_tool_confirmation, is_waiting_context_confirmation, conversation_turn_info) = {
            let ctx = app.chat.lock().unwrap();
            let messages: Vec<_> = ctx.context.iter().cloned().collect();
            let conversation_turn_info = ctx.get_conversation_turn_info();
            (messages, ctx.is_waiting_tool() && !ctx.is_running(), ctx.is_waiting_tool_confirmation(), ctx.is_waiting_context_confirmation(), conversation_turn_info)
        };
        
        // 合并聊天消息和信息消息
        let mut all_messages: Vec<ModelMessage> = Vec::new();
        let mut chat_index = 0;
        let mut info_index = 0;
        
        // 对信息消息按插入位置排序（它们应该已经按添加顺序排序）
        let mut sorted_info_messages = app.info_messages.clone();
        sorted_info_messages.sort_by_key(|&(pos, _)| pos);
        
        // 合并算法：按照插入位置将信息消息插入到聊天消息之间
        while chat_index < messages.len() || info_index < sorted_info_messages.len() {
            // 检查是否有信息消息需要插入到当前位置
            while info_index < sorted_info_messages.len() {
                let (insert_pos, ref info_msg) = sorted_info_messages[info_index];
                if insert_pos <= chat_index {
                    // 插入位置小于等于当前位置，插入信息消息
                    all_messages.push(info_msg.clone());
                    info_index += 1;
                } else {
                    // 插入位置大于当前位置，先处理聊天消息
                    break;
                }
            }
            
            // 处理当前聊天消息
            if chat_index < messages.len() {
                let msg = messages[chat_index].clone();
                // 系统、工具的信息过滤
                if msg.role != "system" && msg.role != "tool" && !msg.content.is_empty() {
                    all_messages.push(msg);
                }
                chat_index += 1;
            }
        }
        
        // 添加剩余的信息消息（如果有）
        while info_index < sorted_info_messages.len() {
            let (_, ref info_msg) = sorted_info_messages[info_index];
            all_messages.push(info_msg.clone());
            info_index += 1;
        }
        
        // 为所有消息创建MessageBlock
        for msg in all_messages {
            Self::add_block(app, MessageBlock::new(msg, app.width));
        }
        
        // 如果工具调用达到上限而中断
        if is_waiting_tool {
            Self::add_system_message_block(app, "工具调用次数达到设置上限，是否继续，输入 yes/y 继续，no/n 中断".into());
        }
        
        // 如果正在等待工具调用确认
        if is_waiting_tool_confirmation {
            Self::add_system_message_block(app, "检测到工具调用，是否执行？输入 yes/y 执行，no/n 取消".into());
        }
        
        // 如果正在等待对话轮次确认
        if is_waiting_context_confirmation {
            let (current_turn, max_turn) = conversation_turn_info;
            Self::add_system_message_block(app, format!("对话轮次已达到上限 ({}/{}), 是否继续？输入 yes/y 继续并重置计数，no/n 停止", current_turn, max_turn));
        }
        
        Self::update_scrollbar_state(app);
    }

    /// 添加系统消息块
    pub fn add_system_message_block(app: &mut App, content: String) {
        Self::add_block(app, MessageBlock::new(ModelMessage::system(content), app.width));
    }

    /// 添加消息块并更新总行数
    pub fn add_block(app: &mut App, block: MessageBlock) {
        app.max_line += block.line_count;
        app.blocks.push(block);
    }

    /// 更新滚动条状态
    pub fn update_scrollbar_state(app: &mut App) {
        if app.max_line > app.window_height {
            app.vertical_scroll_state = app
                .vertical_scroll_state
                .content_length((app.max_line - app.window_height) as usize);
        } else {
            app.vertical_scroll_state = app.vertical_scroll_state.content_length(1);
        }
        app.vertical_scroll_state = app.vertical_scroll_state.position(app.index as usize);
    }
}
