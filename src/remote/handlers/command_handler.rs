//! 处理 GetCommands 请求的处理器

use super::base_handler::RequestHandler;
use crate::chat::Chat;
use crate::config::Config;
use crate::remote::commands::global_registry;
use crate::remote::protocol::{RemoteRequest, RemoteResponse};
use log::info;
use tokio::net::TcpStream;
use tokio_tungstenite::WebSocketStream;

/// 处理 GetCommands 请求的处理器
pub struct CommandHandler;

#[async_trait::async_trait]
impl RequestHandler for CommandHandler {
    async fn handle(
        &self,
        request: RemoteRequest,
        _chat: &mut Chat,
        _config: &Config,
        _ws_stream: &mut WebSocketStream<TcpStream>,
    ) -> RemoteResponse {
        info!("Handling GetCommands request: {}", request.request_id);

        // 获取全局指令注册器
        let registry = global_registry();

        // 构建命令列表
        let mut commands_list = Vec::new();
        for cmd in registry.all() {
            commands_list.push(serde_json::json!({
                "name": cmd.name(),
                "description": cmd.description(),
            }));
        }

        // 创建响应
        let response_json = serde_json::json!({
            "commands": commands_list,
            "count": commands_list.len(),
        });

        RemoteResponse {
            request_id: request.request_id,
            response: crate::remote::protocol::ResponseContent::Text(
                serde_json::to_string(&response_json).unwrap_or_else(|_| {
                    "{\"error\": \"Failed to serialize commands\"}".to_string()
                }),
            ),
            error: None,
            token_usage: None,
        }
    }

    fn can_handle(&self, request: &RemoteRequest) -> bool {
        matches!(
            &request.input,
            crate::remote::protocol::InputType::GetCommands
        )
    }
}
