use std::io::{self, Write};
use std::process::exit;
use clap::{command, Parser};
use futures::{Stream, StreamExt};
use futures::pin_mut;

use crate::chat::StreamedChatResponse;
use crate::prompt::CHAT_PROMPT;
mod config;
mod chat;
mod client;
mod mcp;
mod model;
mod connection;
mod prompt;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// 提示词
    #[arg(short, long)]
    prompt: Option<String>,
    /// 值
    #[arg(short, long, default_value_t = true)]
    stream: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    log4rs::init_file("log4rs.yaml", Default::default()).unwrap();
    mcp::init().await;
    let args = Args::parse();
    if args.stream {
        stream().await;
    }
    else {
        chat().await;
    }

    Ok(())
}

async fn chat() {
    // let args = Args::parse();
    // let config = config::Config::local().unwrap();
    // let mut chat = chat::Chat::new(config, CHAT_PROMPT.to_string())
    // .tools(mcp::get_config_tools())
    // .max_try(3);
    // let res = chat.chat(&args.prompt.unwrap_or_default()).await;
    // if res.is_err() {
    //     println!("{:?}", res.unwrap_err().to_string());
    //     exit(-1);
    // } else {
    //     let res = res.unwrap();
    //     for r in res {
    //         println!("{}{}", r.think, r.content);
    //     }
    // }
}

async fn stream() {
    let args = Args::parse();
    let mut chat = chat::Chat::new(config::Config::local().unwrap(), CHAT_PROMPT.into());
    if args.stream {
        handle_output(chat.stream_chat(&args.prompt.unwrap_or_default())).await;
    } else {
        handle_output(chat.chat(&args.prompt.unwrap_or_default())).await;
    }
}

async fn handle_output(stream: impl Stream<Item = Result<StreamedChatResponse, anyhow::Error>> + '_) {
    pin_mut!(stream);
    while let Some(result) = stream.next().await {
        if let Ok(res) = result {
            match res {
                StreamedChatResponse::Text(text) => print!("{}", text),
                StreamedChatResponse::ToolCall(tool_call) => print!("{}", tool_call),
                StreamedChatResponse::Reasoning(think) => print!("{}", think),
                StreamedChatResponse::ToolResponse(tool) => print!("{}", tool),
            }
            io::stdout().flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use log::info;
    use crate::{chat::Chat, mcp::internalserver::{getbesttool::GetBestTool, InternalTool}, prompt::CHAT_PROMPT};
    use super::*;

    async fn test_select_tool() {
        log4rs::init_file("log4rs.yaml", Default::default()).unwrap();
        mcp::init().await;
        let mut map = serde_json::Map::new();
        map.insert("tool_description".into(), serde_json::Value::String("能够推送仓库到远程的工具".into()));
        let _ = GetBestTool.call(map).await;
    }

    async fn test_search_tool_chat() {
        // log4rs::init_file("log4rs.yaml", Default::default()).unwrap();
        // mcp::init().await;
        // let mut chat = Chat::new(config::Config::local().unwrap(), CHAT_PROMPT.to_string().into())
        // .tools(mcp::get_basic_tools())
        // .max_try(1);
        // let res = chat.chat("你好，帮我查一下github提交信息").await;
        // info!("{:?}", res);
    }
}
