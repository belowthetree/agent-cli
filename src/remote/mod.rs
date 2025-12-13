//! 用于处理 WebSocket 连接以接收用户输入并返回模型响应的远程模块。
//! 
//! 此模块提供了一个 WebSocket 服务器，可以接受来自远程客户端的连接，
//! 接收各种类型的输入（文本、图像、指令等），通过 AI 模型处理它们，
//! 并返回响应。

mod server;
mod client_handler;
mod protocol;
mod commands;

pub use server::RemoteServer;

/// 在指定地址上启动远程 WebSocket 服务器。
pub async fn start_server(addr: &str) -> anyhow::Result<()> {
    // 初始化全局指令注册器
    commands::init_global_registry();
    
    let server = RemoteServer::new(addr).await?;
    server.run().await
}

#[cfg(test)]
mod tests {
    use super::commands::{CommandRegistry, ClearContextCommand};
    use crate::chat::Chat;
    use crate::config::Config;
    use serde_json::json;

    #[tokio::test]
    async fn test_instruction_system() {
        println!("测试远程指令系统...");
        
        // 创建注册器并注册指令
        let mut registry = CommandRegistry::new();
        registry.register(Box::new(ClearContextCommand));
        
        // 测试获取指令列表
        println!("可用指令:");
        for cmd in registry.all() {
            println!("  - {}: {}", cmd.name(), cmd.description());
        }
        
        // 测试查找指令
        let cmd = registry.find("clear_context");
        assert!(cmd.is_some(), "应该能找到 clear_context 指令");
        println!("找到指令: {}", cmd.unwrap().name());
        
        // 测试执行指令
        let config = Config::local().unwrap();
        let mut chat = Chat::new(config);
        
        // 添加一些消息到上下文
        chat.context.push(crate::model::param::ModelMessage::system("系统消息".to_string()));
        chat.context.push(crate::model::param::ModelMessage::user("用户消息1".to_string()));
        chat.context.push(crate::model::param::ModelMessage::assistant("助手消息1".to_string(), "".to_string(), vec![]));
        
        println!("执行前上下文长度: {}", chat.context.len());
        let (turn_count_before, _) = chat.get_conversation_turn_info();
        println!("执行前对话轮次: {}", turn_count_before);
        
        // 执行清理上下文指令
        let result = cmd.unwrap().execute(&mut chat, json!({})).await;
        
        match result {
            Ok(msg) => {
                println!("指令执行成功: {}", msg);
                println!("执行后上下文长度: {}", chat.context.len());
                let (turn_count_after, _) = chat.get_conversation_turn_info();
                println!("执行后对话轮次: {}", turn_count_after);
                
                // 验证上下文已清理
                assert_eq!(chat.context.len(), 1, "上下文应该只保留系统消息");
                assert_eq!(turn_count_after, 0, "对话轮次应该重置为0");
                println!("测试通过!");
            }
            Err(err) => {
                panic!("指令执行失败: {}", err);
            }
        }
    }
}
