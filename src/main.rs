use clap::{command, Parser};
use crate::client::handle_output;
mod config;
mod chat;
mod client;
mod mcp;
mod model;
mod connection;
mod prompt;
mod tui;
#[cfg(feature = "napcat")]
mod napcat;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// 提示词
    #[arg(short, long)]
    prompt: Option<String>,
    /// 是否流式输出（默认流式）
    #[arg(short, long, default_value = "true")]
    stream: Option<bool>,
    /// 是否使用工具（默认使用）
    #[arg(short, long, default_value = "true")]
    use_tool: Option<bool>,
    #[cfg(feature = "napcat")]
    #[arg(short, long, default_value_t = false)]
    napcat: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    log4rs::init_file("log4rs.yaml", Default::default()).unwrap();
    mcp::init().await;
    let args = Args::parse();
    // 优先处理 napcat
    #[cfg(feature = "napcat")]
    if args.napcat {
        napcat::napcat_client::NapCatClient::new(napcat::napcatconfig::NapCatConfig::local().unwrap()).start().await;
        return Ok(())
    }
    if args.prompt.is_none() {
        let _ = tui::run().await;
    }
    else {
        chat().await;
    }
    Ok(())
}

async fn chat() {
    let args = Args::parse();
    let mut chat = chat::Chat::new(config::Config::local().unwrap());
    if Some(true) == args.use_tool {
        chat = chat.tools(mcp::get_config_tools());
    }
    if Some(true) == args.stream {
        handle_output(chat.stream_chat(&args.prompt.unwrap_or_default())).await;
    } else {
        handle_output(chat.chat(&args.prompt.unwrap_or_default())).await;
    }
}

#[cfg(test)]
mod tests {
    use crate::{chat::Chat, mcp::internalserver::{getbesttool::GetBestTool, InternalTool}};
    use super::*;

    #[allow(unused)]
    async fn test_select_tool() {
        log4rs::init_file("log4rs.yaml", Default::default()).unwrap();
        mcp::init().await;
        let mut map = serde_json::Map::new();
        map.insert("tool_description".into(), serde_json::Value::String("能够推送仓库到远程的工具".into()));
        let _ = GetBestTool.call(map).await;
    }

    #[allow(unused)]
    async fn test_search_tool_chat() {
        log4rs::init_file("log4rs.yaml", Default::default()).unwrap();
        mcp::init().await;
        let mut chat = Chat::new(config::Config::local().unwrap())
        .tools(mcp::get_basic_tools())
        .max_try(1);
        let res = chat.chat("你好，帮我查一下github提交信息");
        handle_output(res).await;
    }
}
