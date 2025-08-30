use onebot_v11::{MessageSegment, event::message::GroupMessage};
use serde::{Deserialize, Serialize};
use std::fs;

fn token_default() -> String {
    "123456".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NapCatConfig {
    /// 需要响应的目标用户 QQ
    pub target_qq: Vec<i64>,
    pub self_qq: i64,
    #[serde(default = "token_default")]
    pub token: String,
    pub port: Option<u16>,
}

impl NapCatConfig {
    pub fn local() -> Result<Self, anyhow::Error> {
        let config_content = fs::read_to_string("napcat.toml").expect("找不到 napcat.toml 文件");
        let config_file: Self = toml::from_str(&config_content)?;
        Ok(config_file)
    }
    /// 检查用户是否在目标列表中
    pub fn is_target_user(&self, user_id: i64) -> bool {
        self.target_qq.contains(&user_id)
    }

    pub fn is_group_at_self(&self, msg: GroupMessage) -> bool {
        if !self.is_target_user(msg.user_id) {
            return false;
        }
        let selfqq = self.self_qq.to_string();
        for m in msg.message.iter() {
            if let MessageSegment::At { data } = m {
                if data.qq == selfqq {
                    return true;
                }
            }
        }
        return false;
    }
}
