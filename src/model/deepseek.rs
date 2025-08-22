use crate::connection::{self, CommonConnectionContent};
use crate::model::param::{ModelInputParam, ModelResponse, ToolCall};
use crate::model::AgentModel;
use futures::Stream;
use futures_util::StreamExt;
use log::{debug, warn};
use reqwest::{header, Client};
use rmcp::model::JsonObject;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use futures_util::stream;

// 定义回调函数类型
pub type SseCallback = Box<dyn FnMut(&ModelResponse) + Send>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepseekFunctionItem {
    pub r#type: String,
    pub function: DeepseekFunctionInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepseekFunctionInfo {
    pub name: String,
    pub description: String,
    pub parameters: JsonObject,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepseekModel {
    pub api_key: String,
    pub url: String,
    pub model_name: String,
    pub temperature: String,
}

impl DeepseekModel {
    pub fn new(url: String, model_name: String, api_key: String) -> Self {
        Self {
            api_key,
            url,
            model_name,
            temperature: "0.6".into(),
        }
    }

    fn get_api_key(&self) -> String {
        format!("Bearer {}", self.api_key)
    }

    async fn get_models(&self) -> Result<Vec<String>, String> {
        let client = Client::new();
        let response = client
            .get(format!("{}/models", self.url))
            .header(header::CONTENT_TYPE, "application/json")
            .header("Authorization", self.get_api_key())
            .body(serde_json::to_string(&serde_json::json!({})).unwrap())
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let succ = response.status().is_success();
        let text = response.text().await;
        if succ {
            let text = text.map_err(|e| e.to_string())?;
            let js: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;

            if let Some(message) = js.get("data") {
                let mut res: Vec<String> = Vec::new();
                for val in message.as_array().unwrap().iter() {
                    if let Some(id) = val.get("id") {
                        res.push(serde_json::from_value(id.clone()).unwrap());
                    }
                }
                return Ok(res);
            } else {
                debug!("{:?}", text);
                return Err(text);
            }
        } else {
            debug!("{:?}", text);
            return Err(text.map_err(|e| e.to_string())?);
        }
    }
}

impl AgentModel for DeepseekModel {
    async fn chat(&self, param: ModelInputParam) -> Result<Vec<CommonConnectionContent>, anyhow::Error> {
        let messages = param.messages;
        let mut tools = Vec::new();
        if let Some(ts) = param.tools {
            for tool in ts.iter() {
                let mut p = (*tool.input_schema).clone();
                p.insert("required".into(), json!([]));
                p.insert("type".into(), "object".into());
                let func = DeepseekFunctionItem {
                    r#type: "function".into(),
                    function: DeepseekFunctionInfo {
                        name: tool.name.clone().into(),
                        description: tool.description.as_ref().map(|cow| cow.to_string()).unwrap_or_default(),
                        parameters: p,
                    },
                };
                tools.push(func);
            }
        }
        let body = serde_json::to_string(&serde_json::json!({
            "model": self.model_name,
            "messages": messages,
            "stream": false,
            "tools": if tools.len() > 0 { Some(tools) } else { None },
            "temperature": param.temperature.unwrap_or(self.temperature.parse().unwrap())
        }))
        .unwrap();
        debug!("{:?}", body);
        connection::common::DirectConnection::request(self.url.clone(), self.get_api_key(), body).await
    }

    async fn stream_chat(
        &self,
        param: ModelInputParam,
    ) -> impl Stream<Item = Result<CommonConnectionContent, anyhow::Error>> {
        let messages = param.messages;
        let mut tools = Vec::new();
        // 这里补充两个字段：required type
        if let Some(ts) = param.tools {
            for tool in ts.iter() {
                let mut p = (*tool.input_schema).clone();
                p.insert("required".into(), json!([]));
                p.insert("type".into(), "object".into());
                let func = DeepseekFunctionItem {
                    r#type: "function".into(),
                    function: DeepseekFunctionInfo {
                        name: tool.name.clone().into(),
                        description: tool.description.as_ref().map(|cow| cow.to_string()).unwrap_or_default(),
                        parameters: p,
                    },
                };
                tools.push(func);
            }
        }
        let body = serde_json::to_string(&serde_json::json!({
            "model": self.model_name,
            "messages": messages,
            "stream": true,
            "tools": tools,
            "temperature": param.temperature.unwrap_or(self.temperature.parse().unwrap())
        }))
        .unwrap();
        debug!("{:?}", body);
        connection::common::SseConnection::stream(format!("{}/chat/completions", self.url), self.get_api_key(), body)
    }
}
