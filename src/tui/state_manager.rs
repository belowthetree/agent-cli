use log::debug;

use crate::{
    model::param::ModelMessage,
    tui::{app::App, ui::messageblock::MessageBlock},
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
    /// 4. 为每条消息创建MessageBlock（增量更新）
    /// 5. 计算总行数并更新滚动条状态
    /// 6. 如果工具调用达到上限，显示提示消息
    /// 7. 如果正在等待工具调用确认，显示提示消息
    /// 8. 如果正在等待对话轮次确认，显示提示消息
    pub fn refresh(app: &mut App) {
        let _perf_monitor = crate::perf_start!("StateManager::refresh", 20);
        debug!("refresh");
        
        // 提取需要的信息，然后释放锁
        let (state, conversation_turn_info) = {
            let ctx = {app.chat.lock().unwrap()};
            let conversation_turn_info = ctx.get_conversation_turn_info();
            (ctx.get_state(), conversation_turn_info)
        };
        
        // 增量更新消息块
        Self::update_blocks_incremental(app);
        
        use crate::chat::EChatState;
        match state {
            // 等待工具确认
            EChatState::WaitingToolConfirm => {
                Self::add_system_message_block(app, "检测到工具调用，是否执行？输入 yes/y 执行，no/n 取消".into());
            },
            // 等待轮次确认
            EChatState::WaitingTurnConfirm => {
                let (current_turn, max_turn) = conversation_turn_info;
                Self::add_system_message_block(app, format!("对话轮次已达到上限 ({}/{}), 是否继续？输入 yes/y 继续并重置计数，no/n 停止", current_turn, max_turn));
            },
            _=>{}
        }
        
        Self::update_scrollbar_state(app);
    }
    
    /// 增量更新消息块
    /// 
    /// 比较当前消息列表和已存在的blocks，只更新需要更新的部分
    fn update_blocks_incremental(app: &mut App) {
        let _perf_monitor = crate::perf_start!("StateManager::update_blocks_incremental", 10);
        let width = app.width;
        
        // 如果blocks为空，直接创建所有blocks
        if app.blocks.is_empty() {
            app.blocks.clear();
            app.max_line = 0;
            let messages = app.messages.clone();
            for msg in messages {
                if msg.role != "tool" {
                    Self::add_block(app, MessageBlock::new(msg, width));
                }
            }
            return;
        }
        
        // 计算需要更新的范围
        let old_len = app.blocks.len();
        let new_len = app.messages.len();
        
        // 如果消息数量减少，直接重建
        if new_len < old_len {
            app.blocks.clear();
            app.max_line = 0;
            let messages = app.messages.clone();
            for msg in messages {
                if msg.role != "tool" {
                    Self::add_block(app, MessageBlock::new(msg, width));
                }
            }
            return;
        }
        
        // 找到第一个不同的消息
        let mut first_diff = old_len;
        for i in 0..old_len {
            if !Self::messages_equal(&app.blocks[i].message, &app.messages[i]) {
                first_diff = i;
                break;
            }
        }
        
        // 如果所有现有消息都相同，只添加新消息
        if first_diff == old_len {
            let new_messages: Vec<_> = app.messages[old_len..].iter().cloned().collect();
            for msg in new_messages {
                if msg.role != "tool" {
                    Self::add_block(app, MessageBlock::new(msg, width));
                }
            }
        } else {
            // 从第一个不同的消息开始重建
            app.blocks.truncate(first_diff);
            app.max_line = 0;
            
            // 重新计算前面blocks的高度
            for i in 0..first_diff {
                app.max_line += app.blocks[i].height(width);
            }
            
            // 添加剩余的消息
            let remaining_messages: Vec<_> = app.messages[first_diff..].iter().cloned().collect();
            for msg in remaining_messages {
                if msg.role != "tool" {
                    Self::add_block(app, MessageBlock::new(msg, width));
                }
            }
        }
    }
    
    /// 比较两个消息是否相等
    fn messages_equal(a: &ModelMessage, b: &ModelMessage) -> bool {
        a.role == b.role &&
        a.content == b.content &&
        a.think == b.think &&
        a.name == b.name &&
        a.tool_call_id == b.tool_call_id &&
        a.tool_calls == b.tool_calls &&
        a.token_usage == b.token_usage
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
