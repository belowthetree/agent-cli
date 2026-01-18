use log::info;
use onebot_v11::{MessageSegment, event::message::GroupMessage};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};

fn token_default() -> String {
    "123456".into()
}

fn port_default() -> Option<u16> {
    Some(8082)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NapCatConfig {
    /// 需要响应的目标用户 QQ
    #[serde(default)]
    pub target_qq: Vec<i64>,
    pub self_qq: i64,
    #[serde(default = "token_default")]
    pub token: String,
    #[serde(default = "port_default")]
    pub port: Option<u16>,
}

impl NapCatConfig {
    pub fn local() -> Result<Self, anyhow::Error> {
        // 检查配置文件是否存在
        if !std::path::Path::new("napcat.toml").exists() {
            println!("napcat.toml 配置文件不存在，正在创建默认配置文件...");
            return Self::create_default_config();
        }

        // 读取配置文件
        let config_content = fs::read_to_string("napcat.toml")?;
        let config_file: Self = toml::from_str(&config_content)?;

        // 验证和补全配置字段
        Self::validate_and_complete_config(config_file)
    }

    fn create_default_config() -> Result<Self, anyhow::Error> {
        info!("=== NapCat 配置文件初始化 ===");

        // 获取必要的配置信息
        let self_qq = Self::prompt_user_input_i64("请输入机器人QQ号（必填）: ")?;
        let target_qq_input =
            Self::prompt_user_input_optional("请输入需要响应的目标QQ号（多个用逗号分隔，可选）: ")?;

        let target_qq = if target_qq_input.is_empty() {
            Vec::new()
        } else {
            target_qq_input
                .split(',')
                .map(|s| s.trim().parse::<i64>())
                .filter_map(Result::ok)
                .collect()
        };

        let port_input =
            Self::prompt_user_input_optional("请输入端口号（默认8082，按Enter跳过）: ")?;
        let port = if port_input.is_empty() {
            port_default()
        } else {
            port_input.parse::<u16>().ok()
        };

        // 创建默认配置
        let config = NapCatConfig {
            target_qq,
            self_qq,
            token: token_default(),
            port,
        };

        // 保存配置文件
        let config_toml = toml::to_string_pretty(&config)?;
        fs::write("napcat.toml", config_toml)?;

        println!("配置文件已创建: napcat.toml");
        Ok(config)
    }

    fn validate_and_complete_config(mut config: Self) -> Result<Self, anyhow::Error> {
        let mut needs_save = false;

        // 验证必填字段
        if config.self_qq == 0 {
            println!("机器人QQ号缺失或为0，需要重新输入");
            config.self_qq = Self::prompt_user_input_i64("请输入机器人QQ号: ")?;
            needs_save = true;
        }

        // 设置默认值
        if config.token.is_empty() {
            config.token = token_default();
            needs_save = true;
        }

        if config.port.is_none() {
            config.port = port_default();
            needs_save = true;
        }

        // 如果需要保存，更新配置文件
        if needs_save {
            let config_toml = toml::to_string_pretty(&config)?;
            fs::write("napcat.toml", config_toml)?;
            println!("napcat.toml 配置文件已更新");
        }

        Ok(config)
    }

    fn prompt_user_input_i64(prompt: &str) -> Result<i64, anyhow::Error> {
        loop {
            print!("{}", prompt);
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;

            let input = input.trim();
            if let Ok(value) = input.parse::<i64>() {
                if value != 0 {
                    return Ok(value);
                }
            }

            println!("输入无效，请输入有效的QQ号（非零整数）");
        }
    }

    fn prompt_user_input_optional(prompt: &str) -> Result<String, anyhow::Error> {
        print!("{}", prompt);
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        Ok(input.trim().to_string())
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
