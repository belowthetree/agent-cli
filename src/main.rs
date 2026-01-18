use crate::client::handle_output;
use clap::{Parser, command};
use log::info;
mod acp;
mod chat;
mod client;
mod config;
mod connection;
mod mcp;
mod model;
#[cfg(feature = "napcat")]
mod napcat;
mod prompt;
mod remote;
mod tui;

/// 获取日志配置文件路径
fn get_log_config_path(acp_mode: bool) -> (String, Option<std::path::PathBuf>) {
    if acp_mode {
        // ACP 模式下使用标准数据路径
        if let Some(data_dir) = dirs::data_local_dir() {
            let config_path = data_dir.join("agent-cli").join("log4rs.yaml");
            let log_dir = data_dir.join("agent-cli").join("log");
            return (config_path.to_string_lossy().to_string(), Some(log_dir));
        }
        // 降级到当前目录
        ("log4rs.yaml".to_string(), None)
    } else {
        // 非 ACP 模式使用相对路径
        ("log4rs.yaml".to_string(), None)
    }
}

/// 创建默认的log4rs配置文件
fn create_default_log4rs_config(acp_mode: bool) -> anyhow::Result<()> {
    let (_config_path, log_dir) = get_log_config_path(acp_mode);

    let log_path = if let Some(dir) = log_dir {
        std::fs::create_dir_all(&dir)?;
        dir.join("agent-cli.log").to_string_lossy().to_string()
    } else {
        "log/agent-cli.log".to_string()
    };
    let log_path = log_path.replace("\\", "/");

    let default_config = format!(
        r#"---
# log4rs.yaml
# 检查配置文件变动的时间间隔
refresh_rate: 30 seconds
# appender 负责将日志收集到控制台或文件, 可配置多个
appenders:
  stdout:
    kind: console
  file:
    kind: file
    path: "{}"
    encoder:
      # log 信息模式
      pattern: "[{{d(%Y-%m-%d %H:%M:%S)}}][{{level}}][{{f}}]:{{line}} - {{m}}{{n}}"
# 对全局 log 进行配置
root:
  level: warn
  appenders:
    - file
"#,
        log_path
    );

    if acp_mode {
        // ACP 模式下将配置文件写入标准数据路径
        if let Some(data_dir) = dirs::data_local_dir() {
            let config_path = data_dir.join("agent-cli").join("log4rs.yaml");
            std::fs::create_dir_all(data_dir.join("agent-cli"))?;
            std::fs::write(&config_path, default_config)?;
            info!(
                "已创建ACP模式的log4rs.yaml配置文件到: {}",
                config_path.display()
            );
            return Ok(());
        }
    }

    std::fs::write("log4rs.yaml", default_config)?;
    info!("已创建默认的log4rs.yaml配置文件，全局log等级为warn");
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
    /// 启动ACP模式
    #[arg(long, default_missing_value = "true", num_args = 0..=1)]
    acp: bool,
    /// ACP传输模式（仅对ACP模式有效）：stdio（默认）、wss
    #[arg(long, default_value = "stdio")]
    transport: String,
    /// WSS监听端口（仅对ACP模式有效，默认8338）
    #[arg(long, default_value_t = 8338)]
    wssport: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // ACP 模式下使用标准数据路径的日志配置
    let (log_config_path, _log_dir) = get_log_config_path(args.acp);

    // 检查配置文件是否存在
    let config_path = std::path::Path::new(&log_config_path);
    if !config_path.exists() {
        create_default_log4rs_config(args.acp)?;
    }

    log4rs::init_file(&log_config_path, Default::default()).unwrap();
    // 根据 ACP 模式决定如何加载配置
    info!("{:?}", args);
    let config = if args.acp {
        config::Config::local_with_acp_mode(true).unwrap()
    } else {
        config::Config::local().unwrap()
    };

    mcp::init().await;

    for env in config.envs {
        unsafe {
            std::env::set_var(env.key, env.value);
        }
    }

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

    // 优先处理 ACP 模式
    if args.acp {
        info!("Starting ACP server with transport: {}", args.transport);
        start_acp_server(&args).await?;
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
        handle_output(chat.stream_chat(&args.prompt.unwrap_or_default()))
            .await
            .unwrap();
    } else {
        handle_output(chat.chat(&args.prompt.unwrap_or_default()))
            .await
            .unwrap();
    }
}

/// 启动ACP服务器
async fn start_acp_server(args: &Args) -> anyhow::Result<()> {
    info!("开启 acp");

    use crate::acp::connection::{ConnectionConfig, ConnectionType, run_acp_agent};

    // 确定连接类型
    let connection_type = match args.transport.as_str() {
        "stdio" => ConnectionType::Stdio,
        "wss" => ConnectionType::Wss,
        _ => {
            return Err(anyhow::anyhow!(
                "不支持的传输模式: {}，支持的模式: stdio, wss",
                args.transport
            ));
        }
    };

    // 创建连接配置
    let connection_config = ConnectionConfig {
        connection_type,
        wss_port: if connection_type == ConnectionType::Wss {
            Some(args.wssport)
        } else {
            None
        },
        server_name: "agent-cli".to_string(),
        server_version: env!("CARGO_PKG_VERSION").to_string(),
    };

    info!("ACP Agent 启动，连接类型: {:?}", connection_type);
    if connection_type == ConnectionType::Wss {
        info!("WSS 监听端口: {}", args.wssport);
    }

    // 运行 ACP Agent
    run_acp_agent(connection_config).await?;

    Ok(())
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
        let mut chat = Chat::new(config::Config::local().unwrap()).tools(mcp::get_basic_tools());
        let res = chat.chat("你好，帮我查一下github提交信息");
        handle_output(res).await;
    }
}
