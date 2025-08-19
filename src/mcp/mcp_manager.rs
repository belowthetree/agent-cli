use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex, OnceLock};
use log::{info, log, warn};
use rig::tool::Tool;
use anyhow::Result;
use serde_json::Value;

use crate::config::McpServerTransportConfig;
use crate::mcp::McpTool;

pub struct McpManager {
    services: Arc<Mutex<HashMap<String, McpServerTransportConfig>>>,
    tools: Arc<Mutex<HashMap<String, McpTool>>>,
}

impl McpManager {
    /// 获取全局单例实例
    pub fn global() -> &'static McpManager {
        static INSTANCE: OnceLock<McpManager> = OnceLock::new();
        INSTANCE.get_or_init(|| McpManager {
            services: Arc::new(Mutex::new(HashMap::new())),
            tools: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// 添加工具服务到映射
    pub async fn add_tool_service(&self, server_name: String, transport: McpServerTransportConfig) -> Result<()> {
        let mut services = self.services.lock().map_err(|e| anyhow::anyhow!("Failed to lock tool services: {}", e))?;
        let service = transport.start().await?;
        let tools = service.list_all_tools().await?;
        let mut self_tools = self.tools.lock().map_err(|e| anyhow::anyhow!("解锁 tools 失败 {}", e))?;
        for tool in tools.iter() {
            info!("{:?}", tool);
            // 添加工具顺便检查有无重名
            let mcptool = McpTool::new(
                tool.name.to_string(),
                tool.description.clone().unwrap_or(std::borrow::Cow::Borrowed("")).to_string(),
                Value::Object(tool.input_schema.as_ref().clone()),
                server_name.clone(),
                self_tools.contains_key(&tool.name.to_string())
            );
            self_tools.insert(mcptool.name(), mcptool);
        }
        services.insert(server_name, transport);
        Ok(())
    }

    /// 获取所有工具名称
    pub fn get_all_server_names(&self) -> Result<Vec<String>> {
        let services = self.services.lock().map_err(|e| anyhow::anyhow!("Failed to lock tool services: {}", e))?;
        Ok(services.keys().map(|k| k.clone()).collect())
    }

    /// 检查工具服务是否存在
    pub fn has_tool_service(&self, server_name: &str) -> Result<bool> {
        let services = self.services.lock().map_err(|e| anyhow::anyhow!("Failed to lock tool services: {}", e))?;
        Ok(services.contains_key(server_name))
    }

    /// 清空所有工具服务
    pub fn clear_services(&self) -> Result<()> {
        let mut services = self.services.lock().map_err(|e| anyhow::anyhow!("Failed to lock tool services: {}", e))?;
        services.clear();
        Ok(())
    }

    pub fn call_tool(&self, server_name: &str, tool_name: &str, param: &str)->Result<String> {
        let transport;
        {
            let services = self.services.lock().map_err(|e| anyhow::anyhow!("Failed to lock tool services: {}", e))?;
            transport = services.get(server_name).cloned();
            if transport.is_none() {
                return Err(anyhow::anyhow!("不存在这个 mcp 服务：{}", server_name));
            }
        }
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            if let Ok(service) = transport.unwrap().start().await {
                let value = serde_json::Value::from_str(param).map_err(|e| anyhow::anyhow!("参数 {} 不合法: {}", param, e))?;
                
                if let serde_json::Value::Object(arguments) = value {
                    let result = service.call_tool(rmcp::model::CallToolRequestParam {
                        name: std::borrow::Cow::Owned(tool_name.to_string()),
                        arguments: Some(arguments),
                    }).await?;
                    let mut res = String::new();
                    if result.content.is_none() {
                        return Ok("".to_string())
                    }
                    for v in result.content.unwrap().iter() {
                        match &v.raw {
                            rmcp::model::RawContent::Text(raw_text_content) => {
                                res += raw_text_content.text.as_str();
                            },
                            rmcp::model::RawContent::Image(_) => {
                                warn!("无法处理的 mcp tool 返回类型：图片");
                            },
                            rmcp::model::RawContent::Resource(_) => {
                                warn!("无法处理的 mcp tool 返回类型：嵌入资源");
                            },
                            rmcp::model::RawContent::Audio(_) => {
                                warn!("无法处理的 mcp tool 返回类型：音频");
                            },
                        }
                    }
                    Ok(res)
                } else {
                    Err(anyhow::anyhow!("参数必须是JSON对象"))
                }
            } else {
                Err(anyhow::anyhow!("连接 mcp 服务 {} 失败", server_name))
            }
        })
    }
}
