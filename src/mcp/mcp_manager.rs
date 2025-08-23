use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use log::{info, warn, error};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::McpServerTransportConfig;
use crate::mcp::internalserver::InternalTool;
use crate::mcp::mcp_server::McpService;
use crate::mcp::McpTool;

#[derive(Serialize, Deserialize)]
pub struct ToolDesc {
    pub name: String,
    pub desc: String,
}

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
        let tools = service.list_all_tools().await?;
        let mut self_tools = self.tools.lock().map_err(|e| anyhow::anyhow!("解锁 tools 失败 {}", e))?;
        for tool in tools.iter() {
            // 添加工具顺便检查有无重名
            let mcptool = McpTool::new(
                tool.clone(),
                server_name.clone(),
                self_tools.contains_key(&tool.name.to_string())
            );
            services.insert(mcptool.name(), McpService::from_config(transport.clone()));
            self_tools.insert(mcptool.name(), mcptool);
        }
        let _ = service.notify_initialized().await;
        Ok(())
    }

    pub fn add_internal_tool(&self, tool: Arc<dyn InternalTool>)->Result<()> {
        let mut services = self.services.lock().map_err(|e| anyhow::anyhow!("Failed to lock tool services: {}", e))?;
        let mut self_tools = self.tools.lock().map_err(|e| anyhow::anyhow!("解锁 tools 失败 {}", e))?;
        services.insert(tool.name().to_string(), McpService::from_internal(tool.clone()));
        self_tools.insert(tool.name().to_string(), McpTool::new(tool.get_mcp_tool(), "".into(), false));
        Ok(())
    }

    pub fn remove_tool(&self, name: &str)->Result<()> {
       let mut services = self.services.lock().map_err(|e| anyhow::anyhow!("Failed to lock tool services: {}", e))?;
        services.remove(name);
        let mut self_tools = self.tools.lock().map_err(|e| anyhow::anyhow!("解锁 tools 失败 {}", e))?;
        self_tools.remove(name);
        Ok(())
    }

    pub fn get_all_tools(&self)->Vec<McpTool> {
        let mut res = Vec::new();
        for (_, tool) in self.tools.lock().unwrap().iter() {
            res.push(tool.clone());
        }
        res
    }

    pub fn get_all_tool_desc(&self)->Vec<ToolDesc> {
        let mut res = Vec::new();
        for (_, tool) in self.tools.lock().unwrap().iter() {
            res.push(ToolDesc { name: tool.name(), desc: tool.desc() });
        }
        res
    }

    pub async fn call_tool(&self, tool_name: &str, param: &serde_json::Value)->Result<String> {
        info!("调用工具 {} {:?}", tool_name, param);
        // 工具可能存在循环调用，services 在调用前必须先释放出来
        let service;
        {
            let services = self.services.lock().map_err(|e| anyhow::anyhow!("Failed to lock tool services: {}", e))?;
            let t = services.get(tool_name).cloned();
            if t.is_none() {
                return Err(anyhow::anyhow!("不存在这个 mcp 服务：{}", tool_name));
            }
            service = t.unwrap();
        }

        // 创建一个参数 map
        let arguments_map = if let Value::Object(obj) = param {
            obj.clone()
        } else {
            serde_json::Map::new()
        };

        let result;
        match service {
            // 外部 mcp 工具的调用
            McpService::Common(transport) => {
                let service = transport.start().await;
                if service.is_err() {
                    let e = format!("启动 mcp 服务失败 {:?}", service);
                    error!("{}", e);
                    return Err(anyhow::anyhow!("{}", e));
                }
                if !self.tools.lock().unwrap().contains_key(tool_name) {
                    error!("找不到工具配置 {}", tool_name);
                    return Err(anyhow::anyhow!("找不到工具配置 {}", tool_name));
                }
                let mcptool;
                {
                    mcptool = self.tools.lock().unwrap().get(tool_name).unwrap().clone();
                }
                let service = service.unwrap();
                result = service.call_tool(rmcp::model::CallToolRequestParam {
                    name: std::borrow::Cow::Owned(mcptool.origin_name()),
                    arguments: Some(arguments_map),
                }).await?;
            },
            // 内部定义的工具
            McpService::Internal(internal_tool) => {
                result = internal_tool.call(arguments_map).await?;
            },
        }
        info!("调用工具 {} 结果 {:?}", tool_name, result);
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
    }
}
