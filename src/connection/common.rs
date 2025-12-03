use futures::{Stream};
use async_stream::stream;
use log::{error, info, warn};
use reqwest::header;
use serde_json::Value;
use tokio_stream::StreamExt;

use crate::{connection::{CommonConnectionContent, TokenUsage}, model::param::ToolCall};

pub struct SseConnection;

impl SseConnection {
    pub fn stream(url: String, key: String, body: String)->impl Stream<Item = Result<CommonConnectionContent, anyhow::Error>> {
        stream! {
            let client = reqwest::Client::new();
            let response = client.post(url.clone())
                .header(header::CONTENT_TYPE, "application/json")
                .header("Authorization", key.clone())
                .body(body.clone())
                .send()
                .await
                .map_err(|e| anyhow::anyhow!("{:?}", e.to_string()))?;
            if !response.status().is_success() {
                error!("{:?}", response);
                let ret = response.text().await.unwrap();
                error!("{:?} {:?} {} {}", ret, body, url, key);
                yield Err(anyhow::anyhow!(ret));
                return;
            }
            info!("开始流式处理");
            // 开始循环解析 sse 流式传输
            let mut stream = response.bytes_stream();
            let mut tool_calls = Vec::new();
            while let Some(chunk) = stream.next().await {
                let chunk = match chunk {
                    Ok(chunk) => chunk,
                    Err(e) => {
                        yield Err(anyhow::anyhow!(e.to_string()));
                        continue;
                    }
                };
                let chunk_str = String::from_utf8_lossy(&chunk);

                // 解析SSE格式的数据
                for line in chunk_str.lines() {
                    if !line.starts_with("data:") {
                        continue;
                    }
                    let data = line.trim_start_matches("data:").trim();
                    // 处理结束标志
                    if data == "[DONE]" {
                        for tool in tool_calls {
                            yield Ok(CommonConnectionContent::ToolCall(tool));
                        }
                        return;
                    }
                    
                    let json = serde_json::from_str::<Value>(data);
                    if json.is_err() {
                        continue;
                    }
                    let json = json.unwrap();
                    
                    // 检查是否有 usage 字段（流式响应中通常只在最后发送）
                    if let Some(usage) = json.get("usage") {
                        if let Ok(token_usage) = serde_json::from_value::<TokenUsage>(usage.clone()) {
                            yield Ok(CommonConnectionContent::TokenUsage(token_usage));
                        }
                    }
                    // 获取内容
                    let choices = json.get("choices");
                    if choices.is_none() {
                        continue;
                    }
                    let choices = choices.unwrap();
                    for choice in choices.as_array().unwrap_or(&vec![]) {
                        if let Some(finish_reason) = choice.get("finish_reason") {
                            if finish_reason.as_str() == Some("stop") {
                                yield Ok(CommonConnectionContent::FinishReason("stop".to_string()));
                                for tool in tool_calls {
                                    yield Ok(CommonConnectionContent::ToolCall(tool));
                                }
                                return;
                            }
                        }
                        let mut ret: Option<Value> = None;
                        if let Some(t) = choice.get("message") {
                            ret = Some(t.clone());
                        } else if let Some(t) = choice.get("delta") {
                            ret = Some(t.clone());
                        } else {
                            warn!("未知格式 {:?}", choice);
                        }
                        if ret.is_none() {
                            for tool in tool_calls {
                                yield Ok(CommonConnectionContent::ToolCall(tool));
                            }
                            return;
                        }
                        let message = ret.unwrap();
                        // 处理对话内容
                        if let Some(ctx) = message.get("content") {
                            if let Some(text_str) = ctx.as_str() {
                                yield Ok(CommonConnectionContent::Content(text_str.to_string()));
                            }
                        }
                        // 处理思考
                        if let Some(ctx) = message.get("reasoning_content") {
                            if let Some(text_str) = ctx.as_str() {
                                yield Ok(CommonConnectionContent::Reasoning(text_str.to_string()));
                            }
                        }
                        // 处理工具调用，由于是字段增量的形式接受，等完全接收后再返回
                        if let Some(ctx) = message.get("tool_calls") {
                            if let Some(arr) = ctx.as_array() {
                                for i in 0..arr.len() {
                                    let tool: ToolCall =
                                        serde_json::from_value(arr[i].clone())
                                            .map_err(|e| anyhow::anyhow!(e))?;
                                    if tool_calls.len() <= tool.index {
                                        tool_calls.insert(tool.index, tool);
                                        continue;
                                    }
                                    let t = tool_calls.get_mut(tool.index).unwrap();
                                    t.id += &tool.id;
                                    t.r#type += &tool.r#type;
                                    t.function.name += &tool.function.name;
                                    t.function.arguments +=
                                        &tool.function.arguments;
                                }
                            }
                        }
                    }
                }
            }
            for tool in tool_calls {
                yield Ok(CommonConnectionContent::ToolCall(tool));
            }
        }
    }
}

pub struct DirectConnection;

impl DirectConnection {
    pub async fn request(url: String, key: String, body: String)->Result<Vec<CommonConnectionContent>, anyhow::Error> {
        info!("请求 {}", url);
        let client = reqwest::Client::new();
        let response = client
            .post(url)
            .header(header::CONTENT_TYPE, "application/json")
            .header("Authorization", key)
            .body(body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        if !response.status().is_success() {
            let ret = response.text().await.unwrap();
            error!("{:?}", ret);
            return Err(anyhow::anyhow!(ret));
        }
        info!("请求成功");

        let text = response.text().await.unwrap();
        let json: Value = serde_json::from_str(&text).map_err(|e| anyhow::anyhow!(e))?;

        if let Some(choices) = json.get("choices") {
            if let Some(first_choice) = choices.get(0) {
                if let Some(message) = first_choice.get("message") {
                    let content = message
                        .get("content")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();

                    let tool_calls: Option<Vec<ToolCall>> = message
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
                    let mut res = Vec::new();
                    res.push(CommonConnectionContent::Content(content));
                    if tool_calls.is_some() {
                        let tools = tool_calls.unwrap();
                        for tool in tools.iter() {
                            res.push(CommonConnectionContent::ToolCall(tool.clone()));
                        }
                    }
                    if reasoning_content.is_some() {
                        res.push(CommonConnectionContent::Reasoning(reasoning_content.unwrap()));
                    }
                    if finish_reason.is_some() {
                        res.push(CommonConnectionContent::FinishReason(finish_reason.unwrap()));
                    }
                    
                    // 解析 token 使用情况
                    if let Some(usage) = json.get("usage") {
                        if let Ok(token_usage) = serde_json::from_value::<TokenUsage>(usage.clone()) {
                            res.push(CommonConnectionContent::TokenUsage(token_usage));
                        }
                    }
                    
                    return Ok(res);
                }
            }
        }

        error!("API 请求失败");
        Err(anyhow::anyhow!(text))
    }
}
