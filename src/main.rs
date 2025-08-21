use std::env;
use std::fs::File;
use std::io::{self, Write};
use std::process::exit;
use clap::{command, Parser};
use futures::StreamExt;
use futures::pin_mut;

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
    let mut chat = chat::Chat::new(config);
    // let res = chat.chat(&args.prompt).await;
    // println!("{:?}", res);
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
