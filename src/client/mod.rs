use std::io::{self, Write};

use futures::{pin_mut, Stream, StreamExt};

use crate::chat::StreamedChatResponse;

pub mod chat_client;
pub mod tool_client;

/// 处理流式响应并输出到标准输出
pub async fn handle_output(stream: impl Stream<Item = Result<StreamedChatResponse, anyhow::Error>> + '_) -> anyhow::Result<()> {
    pin_mut!(stream);
    while let Some(result) = stream.next().await {
        match result {
            Ok(res) => {
                match res {
                    StreamedChatResponse::Text(text) => print!("{}", text),
                    StreamedChatResponse::ToolCall(tool_call) => print!("{:?}", tool_call),
                    StreamedChatResponse::Reasoning(think) => print!("{}", think),
                    StreamedChatResponse::ToolResponse(tool) => print!("{:?}", tool),
                    _ => {}
                }
                // 改进错误处理：使用?操作符而不是unwrap()
                io::stdout().flush()?;
            }
            Err(e) => {
                // 记录错误但不中断处理
                eprintln!("处理流式响应时出错: {}", e);
            }
        }
    }
    Ok(())
}

/// 处理单个流式响应项
fn handle_response_item(res: StreamedChatResponse, output: &mut String) {
    match res {
        StreamedChatResponse::Text(text) => {
            print!("{}", text);
            output.push_str(&text);
        }
        StreamedChatResponse::ToolCall(tool_call) => {
            let formatted = format!("{:?}", tool_call);
            print!("{}", formatted);
            output.push_str(&formatted);
        }
        StreamedChatResponse::Reasoning(think) => {
            print!("{}", think);
            output.push_str(&think);
        }
        StreamedChatResponse::ToolResponse(tool) => {
            let formatted = format!("{:?}", tool);
            print!("{}", formatted);
            output.push_str(&formatted);
        }
        _ => {},
    }
}

/// 将流式响应收集为字符串
#[allow(unused)]
pub async fn get_output_tostring(stream: impl Stream<Item = Result<StreamedChatResponse, anyhow::Error>> + '_) -> anyhow::Result<String> {
    pin_mut!(stream);
    let mut output = String::new();
    
    while let Some(result) = stream.next().await {
        match result {
            Ok(res) => {
                handle_response_item(res, &mut output);
                // 刷新标准输出以确保及时显示
                if let Err(e) = io::stdout().flush() {
                    eprintln!("刷新标准输出时出错: {}", e);
                }
            }
            Err(e) => {
                // 将错误信息添加到输出中
                let error_msg = format!("[错误: {}]", e);
                output.push_str(&error_msg);
                eprintln!("{}", error_msg);
            }
        }
    }
    
    Ok(output)
}

/// 处理流式响应并同时收集到字符串
#[allow(unused)]
pub async fn handle_and_collect_output(
    stream: impl Stream<Item = Result<StreamedChatResponse, anyhow::Error>> + '_
) -> anyhow::Result<(String, anyhow::Result<()>)> {
    pin_mut!(stream);
    let mut output = String::new();
    
    while let Some(result) = stream.next().await {
        match result {
            Ok(res) => {
                handle_response_item(res, &mut output);
                if let Err(e) = io::stdout().flush() {
                    return Ok((output, Err(anyhow::anyhow!("刷新标准输出失败: {}", e))));
                }
            }
            Err(e) => {
                let error_msg = format!("[错误: {}]", e);
                output.push_str(&error_msg);
                eprintln!("{}", error_msg);
            }
        }
    }
    
    Ok((output, Ok(())))
}
