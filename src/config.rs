use std::fs;
use rmcp::serde;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap};
use rmcp::{RoleClient, ServiceExt, service::RunningService, transport::ConfigureCommandExt};

// use crate::mcp_adaptor::McpManager;
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct McpServerConfig {
    pub name: String,
    #[serde(default)]
    pub description: String,
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
                        cmd.args(args).envs(envs);
                    }),
                )?;
                ().serve(transport).await?
            }
        };
        Ok(client)
    }
}

fn max_tool_try_default()->usize {
    3
}
fn max_context_num_default()->usize {
    10
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub mcp: Option<McpConfig>,
    pub deepseek_key: String,
    pub url: Option<String>,
    pub model: Option<String>,
    #[serde(default = "max_tool_try_default")]
    pub max_tool_try: usize,
    #[serde(default = "max_context_num_default")]
    pub max_context_num: usize,
}

impl Config {
    pub fn local() -> Result<Self, Box<dyn std::error::Error>> {
        let config_content = fs::read_to_string("config.json").expect("找不到 config.json 文件");
        let config_file: Self = serde_json::from_str(&config_content)?;
        Ok(config_file)
    }
}
