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
        let (state, conversation_turn_info) = {
            let ctx = app.chat.lock().unwrap();
            let conversation_turn_info = ctx.get_conversation_turn_info();
            (ctx.get_state(), conversation_turn_info)
        };
        
        // 为所有消息创建MessageBlock
        let messages = app.messages.clone();
        for msg in messages {
            Self::add_block(app, MessageBlock::new(msg, app.width));
        }

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
