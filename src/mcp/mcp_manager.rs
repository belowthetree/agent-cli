use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use log::{info, warn, error};
use anyhow::Result;
use rmcp::model::RawContent;
use serde_json::Value;

use crate::config::McpServerTransportConfig;
use crate::mcp::mcp_server::McpService;
use crate::mcp::McpTool;

pub struct McpManager {
    services: Arc<Mutex<HashMap<String, McpService>>>,
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
        let prompt = service.list_all_prompts().await;//(GetPromptRequestParam{ name: "simple_prompt".into(), arguments: None }).await;
        info!("prompt {:?}", prompt);
        let tools = service.list_all_tools().await?;
        let mut self_tools = self.tools.lock().map_err(|e| anyhow::anyhow!("解锁 tools 失败 {}", e))?;
        for tool in tools.iter() {
            // 添加工具顺便检查有无重名
            let mcptool = McpTool::new(
                tool.clone(),
                server_name.clone(),
                self_tools.contains_key(&tool.name.to_string())
            );
            self_tools.insert(mcptool.name(), mcptool);
        }
        services.insert(server_name, McpService::from_config(transport));
        let _ = service.notify_initialized().await;
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

    pub fn get_all_tools(&self)->Vec<McpTool> {
        let mut res = Vec::new();
        for (_, tool) in self.tools.lock().unwrap().iter() {
            res.push(tool.clone());
        }
        res
    }

    pub async fn call_tool(&self, server_name: &str, tool_name: &str, param: &serde_json::Value)->Result<String> {
        info!("调用工具 {} {} {:?}", server_name, tool_name, param);
        // 工具可能存在循环调用，services 在调用前必须先释放出来
        let service;
        {
            let services = self.services.lock().map_err(|e| anyhow::anyhow!("Failed to lock tool services: {}", e))?;
            let t = services.get(server_name).cloned();
            if t.is_none() {
                return Err(anyhow::anyhow!("不存在这个 mcp 服务：{}", server_name));
            }
            service = t.unwrap();
        }

        // 创建一个参数 map
        let arguments_map = if let Value::Object(obj) = param {
            obj.clone()
        } else {
            serde_json::Map::new()
        };

        match service {
            // 外部 mcp 工具的调用
            McpService::Common(transport) => {
                let service = transport.start().await;
                if service.is_err() {
                    let e = format!("启动 mcp 服务失败 {:?}", service);
                    error!("{}", e);
                    return Err(anyhow::anyhow!("{}", e));
                }
                let service = service.unwrap();
                let result = service.call_tool(rmcp::model::CallToolRequestParam {
                    name: std::borrow::Cow::Owned(tool_name.to_string()),
                    arguments: Some(arguments_map),
                }).await?;
                info!("调用工具 {} {} 结果 {:?}", server_name, tool_name, result);
                let mut res = String::new();
                if result.content.len() <= 0 {
                    return Ok("".to_string())
                }
                for v in result.content.iter() {
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
            },
            // 内部定义的工具
            McpService::Internal(internal_tool) => {
                let res = internal_tool.call(arguments_map)?;
                let mut rt = String::new();
                for v in res.content {
                    if let RawContent::Text(ct) = v.raw {
                        rt += ct.text.as_str();
                    }
                }
                Ok(rt)
            },
        }
    }
}
