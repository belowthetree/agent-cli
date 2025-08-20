use crate::model::param::{ModelInputParam, ModelResponse, ToolCall};
use crate::model::AgentModel;
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
    pub stream: bool,
}

impl DeepseekModel {
    pub fn new(url: String, model_name: String, api_key: String) -> Self {
        Self {
            api_key,
            url,
            model_name,
            temperature: "0.6".into(),
            stream: true,
        }
    }

    fn get_api_key(&self) -> String {
        format!("Bearer {}", self.api_key)
    }

    async fn generate(
        &self,
        param: ModelInputParam,
        mut stream_callback: Option<SseCallback>,
    ) -> Result<ModelResponse, String> {
        let mut messages = param.messages.unwrap_or_default();

        if messages.is_empty() {
            if param.system.is_some() {
                messages.push(super::param::ModelMessage {
                    role: "system".to_string(),
                    content: param.system.unwrap_or_default(),
                    name: "".into(),
                    tool_call_id: "".into(),
                    tool_calls: None,
                });
            }
            if param.content.is_some() {
                messages.push(super::param::ModelMessage {
                    role: "user".to_string(),
                    content: param.content.unwrap_or_default(),
                    name: "".into(),
                    tool_call_id: "".into(),
                    tool_calls: None,
                });
            }
        }

        let client = reqwest::Client::new();
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
            "stream": self.stream,
            "tools": tools,
            "temperature": param.temperature.unwrap_or(self.temperature.parse().unwrap())
        }))
        .unwrap();
        if messages.len() > 0 {
            debug!("{:?}", messages.last());
        }
        debug!("{:?}", body);
        let response = client
            .post(format!("{}/chat/completions", self.url))
            .header(header::CONTENT_TYPE, "application/json")
            .header("Authorization", self.get_api_key())
            .body(body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let succ = response.status().is_success();
        if !response.status().is_success() {
            let ret = response.text().await.unwrap();
            debug!("{:?}", ret);
            return Err(ret);
        }
        let mut text = "".into();
        if succ {
            // 区分流式和非流式
            if self.stream {
                let mut stream = response.bytes_stream();
                let mut response = ModelResponse {
                    role: "assistant".to_string(),
                    content: "".into(),
                    reasoning_content: None,
                    tool_calls: None,
                    finish_reason: None,
                };

                while let Some(chunk) = stream.next().await {
                    let chunk =
                        chunk.map_err(|e| "stream err".to_string() + e.to_string().as_str())?;
                    let chunk_str = String::from_utf8_lossy(&chunk);

                    // 解析SSE格式的数据
                    for line in chunk_str.lines() {
                        if line.starts_with("data:") {
                            let data = line.trim_start_matches("data:").trim();
                            if data == "[DONE]" {
                                break;
                            }

                            if let Ok(json) = serde_json::from_str::<Value>(data) {
                                if let Some(choices) = json.get("choices") {
                                    for choice in choices.as_array().unwrap_or(&vec![]) {
                                        if let Some(finish_reason) = choice.get("finish_reason") {
                                            if finish_reason.as_str() == Some("stop") {
                                                break;
                                            }
                                        }
                                        if let Some(_) = choice.get("index") {}

                                        let mut ret: Option<Value> = None;
                                        if let Some(t) = choice.get("message") {
                                            ret = Some(t.clone());
                                        } else if let Some(t) = choice.get("delta") {
                                            ret = Some(t.clone());
                                        } else {
                                            warn!("未知格式 {:?}", choice);
                                        }
                                        if let Some(message) = ret {
                                            if let Some(ctx) = message.get("content") {
                                                if let Some(text_str) = ctx.as_str() {
                                                    response.content += text_str;
                                                }
                                            }
                                            if let Some(ctx) = message.get("reasoning_content") {
                                                if let Some(text_str) = ctx.as_str() {
                                                    if !response.reasoning_content.is_none() {
                                                        response.reasoning_content =
                                                            Some("".into());
                                                    }
                                                    response.reasoning_content = Some(
                                                        response.reasoning_content.unwrap()
                                                            + text_str,
                                                    );
                                                }
                                            }
                                            if let Some(ctx) = message.get("tool_calls") {
                                                if let Some(arr) = ctx.as_array() {
                                                    for i in 0..arr.len() {
                                                        let tool: ToolCall =
                                                            serde_json::from_value(arr[i].clone())
                                                                .map_err(|e| e.to_string())?;
                                                        if response.tool_calls.is_none() {
                                                            response.tool_calls = Some(Vec::new());
                                                        }
                                                        let ts =
                                                            response.tool_calls.as_mut().unwrap();
                                                        if ts.len() <= tool.index {
                                                            ts.insert(tool.index, tool);
                                                            continue;
                                                        }
                                                        let t = ts.get_mut(tool.index).unwrap();
                                                        t.id += &tool.id;
                                                        t.r#type += &tool.r#type;
                                                        t.function.name += &tool.function.name;
                                                        t.function.arguments +=
                                                            &tool.function.arguments;
                                                    }
                                                }
                                            }
                                            if let Some(callback) = &mut stream_callback {
                                                callback(&response);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                return Ok(response);
            } else {
                text = response.text().await.unwrap();
                let json: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;

                if let Some(choices) = json.get("choices") {
                    if let Some(first_choice) = choices.get(0) {
                        if let Some(message) = first_choice.get("message") {
                            let content = message
                                .get("content")
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .to_string();

                            let tool_calls = message
                                .get("tool_calls")
                                .and_then(|v| serde_json::from_value(v.clone()).ok());

                            let reasoning_content = message
                                .get("reasoning_content")
                                .and_then(Value::as_str)
                                .map(|s| s.to_string());

                            let finish_reason = first_choice
                                .get("finish_reason")
                                .and_then(Value::as_str)
                                .map(|s| s.to_string());

                            let _ = first_choice.get("index").and_then(Value::as_u64);

                            return Ok(ModelResponse {
                                role: "assistant".to_string(),
                                content,
                                reasoning_content,
                                tool_calls,
                                finish_reason,
                            });
                        }
                    }
                }
            }
        }

        debug!("API 请求失败");
        Err(text)
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
    async fn chat(&self, param: ModelInputParam) -> Result<ModelResponse, String> {
        // 对于非流式聊天，设置stream为false
        let mut model = self.clone();
        model.stream = false;
        model.generate(param, None).await
    }

    async fn stream_chat(
        &self,
        param: ModelInputParam,
    ) -> Result<impl futures_util::Stream<Item = Result<ModelResponse, String>>, String> {
        // 对于流式聊天，设置stream为true
        let mut model = self.clone();
        model.stream = true;

        // 创建一个通道来传递流式响应
        let (tx, rx) = tokio::sync::mpsc::channel(100);

        // 克隆发送器用于在异步任务中使用
        let tx_clone = tx.clone();

        // 在后台运行生成任务
        tokio::spawn(async move {
            let callback: SseCallback = Box::new(move |response| {
                let tx = tx_clone.clone();
                let response = response.clone();
                // 这里我们尝试发送，但如果接收端已关闭则忽略错误
                let _ = tx.try_send(Ok(response));
            });

            match model.generate(param, Some(callback)).await {
                Ok(final_response) => {
                    // 发送最终响应
                    let _ = tx.send(Ok(final_response));
                }
                Err(e) => {
                    // 发送错误
                    let _ = tx.send(Err(e));
                }
            }
        });

        // 创建一个从通道接收的流
        let stream = stream::unfold(rx, |mut rx| async move {
            match rx.recv().await {
                Some(item) => Some((item, rx)),
                None => None,
            }
        });

        Ok(stream)
    }
}
