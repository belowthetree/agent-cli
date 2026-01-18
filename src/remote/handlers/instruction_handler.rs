//! 处理 Instruction 请求的处理器

use super::base_handler::RequestHandler;
use crate::chat::Chat;
use crate::config::Config;
use crate::remote::commands::global_registry;
use crate::remote::protocol::{InputType, RemoteRequest, RemoteResponse};
use log::info;
use tokio::net::TcpStream;
use tokio_tungstenite::WebSocketStream;

/// 处理 Instruction 请求的处理器
pub struct InstructionHandler;

#[async_trait::async_trait]
impl RequestHandler for InstructionHandler {
    async fn handle(
        &self,
        request: RemoteRequest,
        chat: &mut Chat,
        _config: &Config,
        _ws_stream: &mut WebSocketStream<TcpStream>,
    ) -> RemoteResponse {
        let InputType::Instruction {
            command,
            parameters,
        } = &request.input
        else {
            return RemoteResponse::error(
                &request.request_id,
                "Invalid request type for InstructionHandler",
            );
        };

        info!(
            "Handling instruction request: {} - command: {}",
            request.request_id, command
        );

        // 获取全局指令注册器
        let registry = global_registry();

        // 查找指令
        let cmd = match registry.find(command) {
            Some(cmd) => cmd,
            None => {
                return RemoteResponse::error(
                    &request.request_id,
                    &format!("Unknown command: {}", command),
                );
            }
        };

        // 执行指令
        match cmd.execute(chat, parameters.clone()).await {
            Ok(result) => RemoteResponse {
                request_id: request.request_id,
                response: crate::remote::protocol::ResponseContent::Text(result),
                error: None,
                token_usage: None,
            },
            Err(error_msg) => RemoteResponse::error(
                &request.request_id,
                &format!("Command execution failed: {}", error_msg),
            ),
        }
    }

    fn can_handle(&self, request: &RemoteRequest) -> bool {
        matches!(&request.input, InputType::Instruction { .. })
    }
}
