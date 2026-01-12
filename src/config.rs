use anyhow;
use log::info;
use rmcp::serde;
use rmcp::{RoleClient, ServiceExt, service::RunningService};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

// use crate::mcp_adaptor::McpManager;
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct McpServerConfig {
    #[serde(default)]
    pub description: String,
    #[serde(flatten)]
    pub transport: McpServerTransportConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase", untagged)]
pub enum McpServerTransportConfig {
    Streamable {
        url: String,
    },
    Sse {
        sse: String,
    },
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        envs: HashMap<String, String>,
    },
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct McpConfig {
    pub server: HashMap<String, McpServerConfig>,
}

impl McpServerTransportConfig {
    pub async fn start(&self) -> anyhow::Result<RunningService<RoleClient, ()>> {
        let client = match self {
            McpServerTransportConfig::Streamable { url } => {
                let transport =
                    rmcp::transport::StreamableHttpClientTransport::from_uri(url.to_string());
                ().serve(transport).await?
            }
            McpServerTransportConfig::Sse { sse } => {
                let transport = rmcp::transport::SseClientTransport::start(sse.to_string()).await?;
                ().serve(transport).await?
            }
            McpServerTransportConfig::Stdio {
                command,
                args,
                envs,
            } => {
                // 使用管道而不是继承终端，避免破坏crossterm的raw模式输入
                let mut cmd = tokio::process::Command::new(command);
                cmd.args(args)
                    .envs(envs)
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped());
                
                let mut child = cmd.spawn()?;
                
                // 手动获取stdin/stdout管道
                let stdout = child.stdout.take().ok_or_else(|| {
                    anyhow::anyhow!("stdout was not captured")
                })?;
                
                let stdin = child.stdin.take().ok_or_else(|| {
                    anyhow::anyhow!("stdin was not captured")
                })?;
                
                // 创建自定义的transport
                let transport = rmcp::transport::async_rw::AsyncRwTransport::new(stdout, stdin);
                
                // 保存子进程引用以便后续管理
                tokio::spawn(async move {
                    let _ = child.wait().await;
                });
                
                ().serve(transport).await?
            }
        };
        Ok(client)
    }
}

fn max_tool_try_default() -> usize {
    3
}
fn max_context_num_default() -> usize {
    30
}
fn max_tokens_default() -> Option<u32> {
    Some(64000)
}

fn ask_before_tool_execution_default() -> bool {
    false
}

fn auto_compress_threshold_default() -> f32 {
    0.6
}

fn compress_trigger_ratio_default() -> f32 {
    0.7
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EnvConfig {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    #[serde(default)]
    pub mcp: Option<McpConfig>,
    pub api_key: String,
    pub url: Option<String>,
    pub model: Option<String>,
    #[serde(default = "max_tool_try_default")]
    pub max_tool_try: usize,
    #[serde(default = "max_context_num_default")]
    pub max_context_num: usize,
    #[serde(default = "max_tokens_default")]
    pub max_tokens: Option<u32>,
    #[serde(default = "ask_before_tool_execution_default")]
    pub ask_before_tool_execution: bool,
    #[serde(default = "auto_compress_threshold_default")]
    pub auto_compress_threshold: f32,
    #[serde(default = "compress_trigger_ratio_default")]
    pub compress_trigger_ratio: f32,
    pub prompt: Option<String>,
    #[serde(default)]
    pub envs: Vec<EnvConfig>,
}

impl Config {
    fn get_config_dir() -> PathBuf {
        // 获取标准应用配置目录
        #[cfg(target_os = "windows")]
        {
            if let Some(roaming) = std::env::var_os("APPDATA") {
                let path = PathBuf::from(roaming).join("agent-cli");
                return path;
            }
        }
        
        #[cfg(target_os = "macos")]
        {
            if let Some(home) = std::env::var_os("HOME") {
                let path = PathBuf::from(home).join("Library").join("Application Support").join("agent-cli");
                return path;
            }
        }
        
        #[cfg(target_os = "linux")]
        {
            if let Some(home) = std::env::var_os("HOME") {
                let path = PathBuf::from(home).join(".config").join("agent-cli");
                return path;
            }
        }
        
        // 如果无法获取标准路径，回退到当前目录
        PathBuf::from(".")
    }
    
    fn get_config_paths() -> (PathBuf, PathBuf) {
        // 返回两个路径：(检查路径, 创建路径)
        let config_dir = Self::get_config_dir();
        let local_config = PathBuf::from("config.json");
        let app_config = config_dir.join("config.json");
        
        // 优先检查当前目录的配置
        if local_config.exists() {
            (local_config, app_config)
        } else {
            (app_config.clone(), app_config)
        }
    }
    
    pub fn local() -> Result<Self, Box<dyn std::error::Error>> {
        Self::local_with_acp_mode(false)
    }

    pub fn local_with_acp_mode(is_acp_mode: bool) -> Result<Self, Box<dyn std::error::Error>> {
        let (config_path, create_path) = Self::get_config_paths();
        
        // 检查配置文件是否存在
        if !config_path.exists() {
            if is_acp_mode {
                // ACP 模式：从标准应用配置路径创建默认配置（不询问用户）
                return Self::create_default_config_for_acp(&create_path);
            } else {
                println!("配置文件不存在，正在创建默认配置文件...");
                return Self::create_default_config(&create_path);
            }
        }
        
        // 读取配置文件
        let config_content = fs::read_to_string(&config_path)?;
        let mut config_file: Self = serde_json::from_str(&config_content)?;
        
        // 验证和补全配置字段
        if is_acp_mode {
            // ACP 模式：不询问用户，直接使用默认值补全缺失配置
            config_file = Self::complete_config_with_defaults(config_file);
        } else {
            config_file = Self::validate_and_complete_config(config_file, &create_path)?;
        }
        
        Ok(config_file)
    }
    
    fn create_default_config(config_path: &PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        info!("=== 配置文件初始化 ===");
        
        // 确保配置目录存在
        if let Some(parent) = config_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
                println!("配置目录已创建: {}", parent.display());
            }
        }
        
        // 获取必要的配置信息
        let api_key = Self::prompt_user_input("请输入API密钥（必填）: ")?;
        let url = Self::prompt_user_input_optional("请输入API URL（可选，按Enter跳过）: ")?;
        let model = Self::prompt_user_input_optional("请输入模型名称（可选，按Enter跳过）: ")?;
        
        // 创建默认配置
        let config = Config {
            mcp: None,
            api_key,
            url: if url.is_empty() { None } else { Some(url) },
            model: if model.is_empty() { None } else { Some(model) },
            max_tool_try: max_tool_try_default(),
            max_context_num: max_context_num_default(),
            max_tokens: max_tokens_default(),
            ask_before_tool_execution: ask_before_tool_execution_default(),
            auto_compress_threshold: auto_compress_threshold_default(),
            compress_trigger_ratio: compress_trigger_ratio_default(),
            prompt: None,
            envs: Vec::new(),
        };
        
        // 保存配置文件
        let config_json = serde_json::to_string_pretty(&config)?;
        fs::write(config_path, config_json)?;
        
        println!("配置文件已创建: {}", config_path.display());
        Ok(config)
    }

    /// 为 ACP 模式创建默认配置（不询问用户）
    fn create_default_config_for_acp(config_path: &PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        info!("=== ACP 模式：创建默认配置文件 ===");
        
        // 确保配置目录存在
        if let Some(parent) = config_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
                info!("配置目录已创建: {}", parent.display());
            }
        }
        
        // 创建默认配置（使用空字符串作为占位符）
        let config = Config {
            mcp: None,
            api_key: String::new(), // 空字符串，需要由客户端提供
            url: None,
            model: None,
            max_tool_try: max_tool_try_default(),
            max_context_num: max_context_num_default(),
            max_tokens: max_tokens_default(),
            ask_before_tool_execution: ask_before_tool_execution_default(),
            auto_compress_threshold: auto_compress_threshold_default(),
            compress_trigger_ratio: compress_trigger_ratio_default(),
            prompt: None,
            envs: Vec::new(),
        };
        
        // 保存配置文件
        let config_json = serde_json::to_string_pretty(&config)?;
        fs::write(config_path, config_json)?;
        
        info!("配置文件已创建: {}", config_path.display());
        Ok(config)
    }

    /// 使用默认值补全配置（不询问用户）
    fn complete_config_with_defaults(mut config: Self) -> Self {
        // 设置默认值
        if config.max_tool_try == 0 {
            config.max_tool_try = max_tool_try_default();
        }
        
        if config.max_context_num == 0 {
            config.max_context_num = max_context_num_default();
        }
        
        if config.max_tokens.is_none() {
            config.max_tokens = max_tokens_default();
        }
        
        config
    }
    
    fn validate_and_complete_config(mut config: Self, config_path: &PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        let mut needs_save = false;
        
        // 验证必填字段
        if config.api_key.is_empty() {
            println!("API密钥缺失，需要重新输入");
            config.api_key = Self::prompt_user_input("请输入API密钥: ")?;
            needs_save = true;
        }
        
        // 设置默认值
        if config.max_tool_try == 0 {
            config.max_tool_try = max_tool_try_default();
            needs_save = true;
        }
        
        if config.max_context_num == 0 {
            config.max_context_num = max_context_num_default();
            needs_save = true;
        }
        
        if config.max_tokens.is_none() {
            config.max_tokens = max_tokens_default();
            needs_save = true;
        }
        
        // 如果需要保存，更新配置文件
        if needs_save {
            let config_json = serde_json::to_string_pretty(&config)?;
            fs::write(config_path, config_json)?;
            println!("配置文件已更新: {}", config_path.display());
        }
        
        Ok(config)
    }
    
    fn prompt_user_input(prompt: &str) -> Result<String, Box<dyn std::error::Error>> {
        loop {
            print!("{}", prompt);
            io::stdout().flush()?;
            
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            
            let input = input.trim().to_string();
            if !input.is_empty() {
                return Ok(input);
            }
            
            println!("输入不能为空，请重新输入");
        }
    }
    
    fn prompt_user_input_optional(prompt: &str) -> Result<String, Box<dyn std::error::Error>> {
        print!("{}", prompt);
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        Ok(input.trim().to_string())
    }
}
