use crate::client::handle_output;
use clap::{Parser, command};
use log::info;
mod chat;
mod client;
mod config;
mod connection;
mod mcp;
mod model;
#[cfg(feature = "napcat")]
mod napcat;
mod prompt;
mod tui;
mod remote;

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
    /// 是否等待用户输入（默认不等待）
    #[arg(short, long, default_value = "false")]
    wait: Option<bool>,
    /// 启动远程TCP服务器
    #[arg(long)]
    remote: Option<String>,
    #[cfg(feature = "napcat")]
    #[arg(short, long, default_value_t = false)]
    napcat: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    log4rs::init_file("log4rs.yaml", Default::default()).unwrap();
    mcp::init().await;
    let config = config::Config::local().unwrap();
    for env in config.envs {
        unsafe {
            std::env::set_var(env.key, env.value);
        }
    }
    let args = Args::parse();
    
    // 优先处理 remote 模式
    if let Some(addr) = args.remote {
        info!("Starting remote server on {}", addr);
        remote::start_server(&addr).await?;
        return Ok(());
    }
    
    // 优先处理 napcat
    #[cfg(feature = "napcat")]
    if args.napcat {
        napcat::napcat_client::NapCatClient::new(
            napcat::napcatconfig::NapCatConfig::local().unwrap(),
        )
        .start()
        .await;
        return Ok(());
    }

    // 处理 wait 模式
    if Some(true) == args.wait {
        wait_mode(args).await;
    } else if args.prompt.is_none() {
        let _ = tui::run().await;
    } else {
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
        handle_output(chat.stream_chat(&args.prompt.unwrap_or_default())).await.unwrap();
    } else {
        handle_output(chat.chat(&args.prompt.unwrap_or_default())).await.unwrap();
    }
}

async fn wait_mode(args: Args) {
    use std::io::{self, BufRead, Write};

    println!("进入等待模式，输入 'exit' 或 'quit' 退出");
    println!("请输入您的消息:");

    let stdin = io::stdin();
    let mut stdin_lock = stdin.lock();
    let mut buffer = String::new();

    loop {
        buffer.clear();
        print!("> ");
        io::stdout().flush().unwrap();

        if stdin_lock.read_line(&mut buffer).is_err() {
            println!("读取输入失败");
            break;
        }

        let input = buffer.trim();
        if input.is_empty() {
            continue;
        }

        if input.eq_ignore_ascii_case("exit") || input.eq_ignore_ascii_case("quit") {
            println!("退出等待模式");
            break;
        }

        // 创建新的 Chat 实例，不保存上下文
        let mut chat = chat::Chat::new(config::Config::local().unwrap());
        if Some(true) == args.use_tool {
            chat = chat.tools(mcp::get_config_tools());
        }

        if Some(true) == args.stream {
            handle_output(chat.stream_chat(input)).await.unwrap();
        } else {
            handle_output(chat.chat(input)).await.unwrap();
        }

        println!(); // 添加空行分隔每次对话
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        chat::Chat,
        mcp::internalserver::{InternalTool, getbesttool::GetBestTool},
    };

    #[allow(unused)]
    async fn test_select_tool() {
        log4rs::init_file("log4rs.yaml", Default::default()).unwrap();
        mcp::init().await;
        let mut map = serde_json::Map::new();
        map.insert(
            "tool_description".into(),
            serde_json::Value::String("能够推送仓库到远程的工具".into()),
        );
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
