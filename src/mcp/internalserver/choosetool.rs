use std::sync::Arc;

use async_trait::async_trait;
use rmcp::model::{Annotated, CallToolResult, RawContent, RawTextContent, Tool};
use serde_json::{Map, Value};

use crate::mcp::internalserver::InternalTool;

#[derive(Debug)]
pub struct ChooseTool;

#[async_trait]
impl InternalTool for ChooseTool {
    async fn call(&self, args: Map<String, Value>)->anyhow::Result<CallToolResult> {
        Ok(CallToolResult {
            content: vec![Annotated::new(
                RawContent::Text(RawTextContent {
                    text: serde_json::to_string(&args).unwrap(),
                }),
                None,
            )],
            structured_content: None,
            is_error: None,
        })
    }

    fn get_mcp_tool(&self)->Tool {
        Tool{
            name: "choose_tool".into(),
            description: Some("Tell system and user the most appropriate tools should be use".into()),
            input_schema: serde_json::from_str(
                r#"
{
    "properties":{
        "tools":{
            "description":"tools you choose",
            "type": "array",
            "items": {
                "type": "string"
            }
        }
    }
}"#).unwrap(),
            output_schema: None,
            annotations: None,
        }
    }
}