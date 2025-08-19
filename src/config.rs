use std::fs;
use rmcp::serde;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, process::Stdio};
use rmcp::{RoleClient, ServiceExt, service::RunningService, transport::ConfigureCommandExt};

// use crate::mcp_adaptor::McpManager;
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct McpServerConfig {
    pub name: String,
    #[serde(flatten)]
    pub transport: McpServerTransportConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "protocol", rename_all = "lowercase")]
pub enum McpServerTransportConfig {
    Streamable {
        url: String,
    },
    Sse {
        url: String,
    },
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        envs: HashMap<String, String>,
    },
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct McpConfig {
    pub server: Vec<McpServerConfig>,
}

impl McpServerTransportConfig {
    pub async fn start(&self) -> anyhow::Result<RunningService<RoleClient, ()>> {
        let client = match self {
            McpServerTransportConfig::Streamable { url } => {
                let transport =
                    rmcp::transport::StreamableHttpClientTransport::from_uri(url.to_string());
                ().serve(transport).await?
            }
            McpServerTransportConfig::Sse { url } => {
                let transport = rmcp::transport::SseClientTransport::start(url.to_string()).await?;
                ().serve(transport).await?
            }
            McpServerTransportConfig::Stdio {
                command,
                args,
                envs,
            } => {
                let transport = rmcp::transport::TokioChildProcess::new(
                    tokio::process::Command::new(command).configure(|cmd| {
                        cmd.args(args).envs(envs).stderr(Stdio::null());
                    }),
                )?;
                ().serve(transport).await?
            }
        };
        Ok(client)
    }
}


#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub mcp: Option<McpConfig>,
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
