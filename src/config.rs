use std::fs;

use crate::mcp;
use rmcp::serde;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub mcp: Option<mcp::McpConfig>,
    pub deepseek_key: String,
    pub deepseek_base_url: Option<String>,
}

impl Config {
    pub fn local() -> Result<Self, Box<dyn std::error::Error>> {
        let config_content = fs::read_to_string("config.toml").expect("找不到 config.toml 文件");

        let config_file: Self = toml::from_str(&config_content)?;

        Ok(Config {
            mcp: None,
            deepseek_key: config_file.deepseek_key,
            deepseek_base_url: config_file.deepseek_base_url,
        })
    }
}