use std::fs::File;
use std::io::{self, Write};
use std::process::exit;
use clap::{command, Parser};
use futures::StreamExt;
use futures::pin_mut;

use crate::mcp::internalserver::getbesttool::GetBestTool;
use crate::mcp::internalserver::InternalTool;
use crate::mcp::{mcp_manager, McpManager};
mod config;
mod chat;
mod client;
mod mcp;
mod model;
mod connection;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// 提示词
    #[arg(short, long)]
    prompt: String,

    /// 值
    #[arg(short, long, default_value_t = String::new())]
    value: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    log4rs::init_file("log4rs.yaml", Default::default()).unwrap();

    mcp::init().await;
    let args = Args::parse();
    let config = config::Config::local().unwrap();
    let mut chat = chat::Chat::new(config, "你是一个优秀的助理，你擅长替人解决问题，必要时可以灵活使用工具".into());
    let res = chat.chat(&args.prompt).await;
    println!("{:?}", res);
    // let stream = chat.stream_chat(&args.prompt);

    // pin_mut!(stream);
    // while let Some(result) = stream.next().await {
    //     match result {
    //         Ok(resp) => match resp {
    //             client::chat_client::StreamedChatResponse::Text(text) => {
    //                 print!("{}", text);
    //                 io::stdout().flush()?;
    //             }
    //             client::chat_client::StreamedChatResponse::ToolCall(tool_call) => {
    //                 println!("\nTool Call: {:?}", tool_call.function.name);
    //             }
    //             client::chat_client::StreamedChatResponse::Reasoning(reasoning) => {
    //                 print!("\nThinking: {}", reasoning);
    //             }
    //         },
    //         Err(e) => {
    //             eprintln!("\n接收错误: {}", e);
    //             break;
    //         }
    //     }
    // }

    Ok(())
}

#[cfg(test)]
mod tests {
    use log::info;
    use crate::chat::Chat;
    use super::*;

    async fn test_select_tool() {
        log4rs::init_file("log4rs.yaml", Default::default()).unwrap();
        mcp::init().await;
        let mut map = serde_json::Map::new();
        map.insert("tool_description".into(), serde_json::Value::String("能够推送仓库到远程的工具".into()));
        let _ = GetBestTool.call(map).await;
    }

    #[tokio::test]
    async fn test_common_chat() {
        log4rs::init_file("log4rs.yaml", Default::default()).unwrap();
        mcp::init().await;
        let mut chat = Chat::new(config::Config::local().unwrap(), "你是一个优秀的助理，你擅长替人解决问题，必要时可以灵活使用工具。对于用户的提问或者对话请认真回答".into())
        .tools(mcp::get_basic_tools())
        .max_try(1);
        let res = chat.chat("你好，帮我查一下github提交信息").await;
        info!("{:?}", res);
    }
}
