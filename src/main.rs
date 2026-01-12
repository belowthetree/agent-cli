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
mod acp;

/// 创建默认的log4rs配置文件
fn create_default_log4rs_config() -> anyhow::Result<()> {
    let default_config = r#"---
# log4rs.yaml
# 检查配置文件变动的时间间隔
refresh_rate: 30 seconds
# appender 负责将日志收集到控制台或文件, 可配置多个
appenders:
  stdout:
    kind: console
  file:
    kind: file
    path: "log/agent-cli.log"
    encoder:
      # log 信息模式
      pattern: "[{d(%Y-%m-%d %H:%M:%S)}][{level}][{f}]:{line} - {m}{n}"
# 对全局 log 进行配置
root:
  level: warn
  appenders:
    - file
"#;

    std::fs::write("log4rs.yaml", default_config)?;
    println!("已创建默认的log4rs.yaml配置文件，全局log等级为warn");
    Ok(())
}

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
    /// 启动远程WebSocket服务器
    #[arg(long, default_missing_value = "0.0.0.0:3838", num_args = 0..=1)]
    remote: Option<String>,
    #[cfg(feature = "napcat")]
    #[arg(short, long, default_value_t = false)]
    napcat: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 检查log4rs.yaml是否存在，如果不存在则创建默认配置
    if !std::path::Path::new("log4rs.yaml").exists() {
        create_default_log4rs_config()?;
    }
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
            .tools(mcp::get_basic_tools());
        let res = chat.chat("你好，帮我查一下github提交信息");
        handle_output(res).await;
    }
}
