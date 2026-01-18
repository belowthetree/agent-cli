#![allow(unused)]
pub enum PostType {
    Meta,        // 元事件
    Request,     // 请求事件
    Notice,      // 通知事件
    Message,     // 消息事件
    MessageSent, // 消息发送事件
}

impl PostType {
    pub fn value(&self) -> &str {
        match self {
            PostType::Meta => "meta_event",
            PostType::Request => "request",
            PostType::Notice => "notice",
            PostType::Message => "message",
            PostType::MessageSent => "message_sent",
        }
    }
}

pub enum NoticeType {
    GroupUpload,       // 群文件上传
    GroupAdmin,        // 群管理员变动
    GroupDecrease,     // 群成员减少
    GroupIncrease,     // 群成员增加
    GroupBan,          // 群禁言
    FriendAdd,         // 新添加好友
    GroupRecall,       // 群消息撤回
    FriendRecall,      // 好友消息撤回
    Poke,              // 戳一戳
    LuckyKing,         // 运气王
    Honor,             // 荣誉变更
    GroupMsgEmojiLike, // 群表情回应
    Essence,           // 群精华
    GroupCard,         // 群名片变更
}

impl NoticeType {
    pub fn value(&self) -> &str {
        match self {
            NoticeType::GroupUpload => "group_upload",
            NoticeType::GroupAdmin => "group_admin",
            NoticeType::GroupDecrease => "group_decrease",
            NoticeType::GroupIncrease => "group_increase",
            NoticeType::GroupBan => "group_ban",
            NoticeType::FriendAdd => "friend_add",
            NoticeType::GroupRecall => "group_recall",
            NoticeType::FriendRecall => "friend_recall",
            NoticeType::Poke => "poke",
            NoticeType::LuckyKing => "lucky_king",
            NoticeType::Honor => "honor",
            NoticeType::GroupMsgEmojiLike => "group_msg_emoji_like",
            NoticeType::Essence => "essence",
            NoticeType::GroupCard => "group_card",
        }
    }
}
