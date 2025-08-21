use async_trait::async_trait;
use rmcp::model::{Annotated, RawContent, RawTextContent};

use crate::{chat::Chat, config, mcp::{internalserver::InternalTool, McpManager}};

#[derive(Debug)]
pub struct GetBestTool;

const PROMPT: &'static str = "你是一个工具查询系统，请根据用户输入的信息找到所有可能用上的工具，并使用工具 choose_tool 返回你选择的工具";

#[async_trait]
impl InternalTool for GetBestTool {
    async fn call(&self, args: serde_json::Map<String, serde_json::Value>)->anyhow::Result<rmcp::model::CallToolResult> {
        if !args.contains_key("tool_description") {
            return Err(anyhow::anyhow!("GetBestToo 缺少参数 tool_description"));
        }
        let prompt = args.get("tool_description").unwrap().as_str().unwrap();
        let mut config = config::Config::local().unwrap();
        config.max_tool_try = 1;
        let tools = McpManager::global().get_all_tools();
        let s = serde_json::to_string(&tools).unwrap();
        let mut chat = Chat::new(config, PROMPT.to_string() + " 以下是工具列表：\n" + s.as_str());
        let res = chat.chat_with_tools(prompt, &vec![self.get_mcp_tool()]).await?;
        let mut arr: Vec<String> = Vec::new();
        for r in res.iter() {
            arr.push(serde_json::from_str(r.content.as_str())?);
        }
        let mut res = Vec::new();
        for s in arr {
            res.push(Annotated::new(RawContent::Text(RawTextContent { text: s }), None));
        }
        println!("{:?}", res);
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
}