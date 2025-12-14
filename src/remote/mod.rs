//! 用于处理 WebSocket 连接以接收用户输入并返回模型响应的远程模块。
//! 
//! 此模块提供了一个 WebSocket 服务器，可以接受来自远程客户端的连接，
//! 接收各种类型的输入（文本、图像、指令等），通过 AI 模型处理它们，
//! 并返回响应。

mod server;
mod client_handler;
mod protocol;
mod commands;
mod handlers;
mod shared;

pub use server::RemoteServer;

/// 在指定地址上启动远程 WebSocket 服务器。
pub async fn start_server(addr: &str) -> anyhow::Result<()> {
    // 初始化全局指令注册器
    commands::init_global_registry();
    
    let server = RemoteServer::new(addr).await?;
    server.run().await
}
