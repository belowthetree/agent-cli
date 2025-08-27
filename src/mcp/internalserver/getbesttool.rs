use std::sync::Arc;

use async_trait::async_trait;
use futures::{pin_mut, StreamExt};
use log::info;
use rmcp::model::{Annotated, RawContent, RawTextContent};

use crate::{chat::Chat, config, mcp::{internalserver::{choosetool::ChooseTool, InternalTool}, McpManager, McpTool}};

#[allow(unused)]
#[derive(Debug)]
pub struct GetBestTool;

const PROMPT: &'static str = "你是一个工具查询系统，请根据用户输入的信息找到所有可能用上的工具，并使用工具 choose_tool 返回你选择的工具";

#[async_trait]
impl InternalTool for GetBestTool {
    async fn call(&self, args: serde_json::Map<String, serde_json::Value>)->anyhow::Result<rmcp::model::CallToolResult> {
        if !args.contains_key("tool_description") {
            return Err(anyhow::anyhow!("GetBestToo 缺少参数 tool_description"));
        }
        // 先新建一个对话
        let prompt = args.get("tool_description").unwrap().as_str().unwrap();
        let mut config = config::Config::local().unwrap();
        config.max_tool_try = 0;
        let tools = McpManager::global().get_all_tool_desc();
        let s = serde_json::to_string(&tools).unwrap();
        let system = PROMPT.to_string() + " 以下是工具列表：\n" + s.as_str();
        config.prompt = Some(system);
        info!("prompt {}", prompt);
        // 把“选择工具”的接口传给 mcp_manager 和对话器
        let tool = ChooseTool.get_mcp_tool();
        McpManager::global().add_internal_tool(Arc::new(ChooseTool))?;
        let mut chat = Chat::new(config)
        .tools(vec![McpTool::new(tool.clone(), "".into(), false)]);
        // 开始获取结果
        let stream = chat.chat(prompt);
        pin_mut!(stream);
        let mut result = String::new();
        while let Some(tmp) = stream.next().await {
            if let Ok(t) = tmp {
                match t {
                    crate::chat::StreamedChatResponse::Text(text) => {
                        result = text;
                        break;
                    },
                    _ => {},
                }
            }
        }
        // 获取完毕后清理下临时接口
        McpManager::global().remove_tool(&tool.name)?;
        info!("获取最佳工具：{:?}", result);
        let mut res = Vec::new();
        res.push(Annotated::new(RawContent::Text(RawTextContent { text: result }), None));
        Ok(rmcp::model::CallToolResult {
            content: res,
            structured_content: None,
            is_error: None,
        })
    }

    fn get_mcp_tool(&self)->rmcp::model::Tool {
        rmcp::model::Tool{
            name: "get_best_tool".into(),
            description: Some(std::borrow::Cow::Borrowed("获取你最需要的工具信息")),
            input_schema: serde_json::from_str(
r#"
{
    "properties":{
        "tool_description":{
            "description":"描述你需要的工具信息",
            "type": "string"
        }
    }
}"#).unwrap(),
            output_schema: Some(serde_json::from_str(
r#"
{
    "properties":{
        "tools":{
            "description":"工具的名字数组",
            "type": "array",
            "items": {
                "type": "string"
            }
        }
    }
}"#).unwrap()),
            annotations: None,
        }
    }

    fn name(&self)->String {
        "get_best_tool".into()
    }
}
