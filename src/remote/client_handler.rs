//! 用于处理单个客户端连接的客户端处理器。

use crate::chat::Chat;
use crate::config::Config;
use futures::{SinkExt, StreamExt};
use log::{error, info};
use tokio::net::TcpStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;

use super::handlers::HandlerFactory;
use super::protocol::{RemoteRequest, RemoteResponse};

/// 用于处理单个客户端连接的处理器。
pub struct ClientHandler {
    ws_stream: WebSocketStream<TcpStream>,
    config: Config,
    chat: Chat,
}

impl ClientHandler {
    /// 创建一个新的客户端处理器。
    pub fn new(ws_stream: WebSocketStream<TcpStream>, config: Config) -> Self {
        Self {
            ws_stream,
            config: config.clone(),
            chat: Chat::new(config),
        }
    }

    /// 处理客户端连接。
    pub async fn handle(&mut self) -> anyhow::Result<()> {
        info!("New WebSocket client connected");

        loop {
            match self.ws_stream.next().await {
                Some(Ok(message)) => {
                    match message {
                        Message::Text(text) => {
                            info!("Received WebSocket message: {}", text);

                            // Parse the request
                            let request: RemoteRequest = match serde_json::from_str(&text) {
                                Ok(req) => req,
                                Err(e) => {
                                    error!("Failed to parse request: {} {}", e, text);
                                    let error_response = RemoteResponse::error(
                                        "parse_error",
                                        &format!("Invalid request format: {}", e),
                                    );
                                    let error_json = serde_json::to_string(&error_response)?;
                                    self.ws_stream.send(Message::Text(error_json)).await?;
                                    continue;
                                }
                            };

                            // Check if this is an interrupt request while chat is running
                            if let super::protocol::InputType::Interrupt = &request.input {
                                // Handle interrupt immediately
                                let response = self.handle_interrupt(&request.request_id);
                                let response_json = serde_json::to_string(&response)?;
                                self.ws_stream.send(Message::Text(response_json)).await?;
                                continue;
                            }

                            // Process the request using the handler architecture
                            let response = self.process_request_with_handler(request).await;

                            // Send the response
                            let response_json = serde_json::to_string(&response)?;
                            self.ws_stream.send(Message::Text(response_json)).await?;
                        }
                        Message::Binary(data) => {
                            info!("Received binary message ({} bytes)", data.len());
                            // Try to parse as JSON string
                            if let Ok(text) = String::from_utf8(data) {
                                match serde_json::from_str::<RemoteRequest>(&text) {
                                    Ok(request) => {
                                        // Check if this is an interrupt request while chat is running
                                        if let super::protocol::InputType::Interrupt =
                                            &request.input
                                        {
                                            // Handle interrupt immediately
                                            let response =
                                                self.handle_interrupt(&request.request_id);
                                            let response_json = serde_json::to_string(&response)?;
                                            self.ws_stream
                                                .send(Message::Text(response_json))
                                                .await?;
                                            continue;
                                        }

                                        let response =
                                            self.process_request_with_handler(request).await;
                                        let response_json = serde_json::to_string(&response)?;
                                        self.ws_stream.send(Message::Text(response_json)).await?;
                                    }
                                    Err(e) => {
                                        error!("Failed to parse binary message as JSON: {}", e);
                                        let error_response = RemoteResponse::error(
                                            "parse_error",
                                            &format!("Invalid request format: {}", e),
                                        );
                                        let error_json = serde_json::to_string(&error_response)?;
                                        self.ws_stream.send(Message::Text(error_json)).await?;
                                    }
                                }
                            } else {
                                error!("Binary message is not valid UTF-8");
                                let error_response = RemoteResponse::error(
                                    "parse_error",
                                    "Binary message must be valid UTF-8 JSON",
                                );
                                let error_json = serde_json::to_string(&error_response)?;
                                self.ws_stream.send(Message::Text(error_json)).await?;
                            }
                        }
                        Message::Ping(data) => {
                            info!("Received ping, sending pong");
                            self.ws_stream.send(Message::Pong(data)).await?;
                        }
                        Message::Pong(_) => {
                            // Ignore pong messages
                        }
                        Message::Close(_frame) => {
                            info!("Received close frame: {:?}", _frame);
                            if let Some(frame) = _frame {
                                self.ws_stream.send(Message::Close(Some(frame))).await?;
                            } else {
                                self.ws_stream.send(Message::Close(None)).await?;
                            }
                            break;
                        }
                        Message::Frame(_) => {
                            // Raw frame, we don't handle this directly
                            // It's handled internally by tungstenite
                        }
                    }
                }
                Some(Err(e)) => {
                    error!("WebSocket error: {}", e);
                    break;
                }
                None => {
                    info!("WebSocket connection closed by client");
                    break;
                }
            }
        }

        Ok(())
    }

    /// 使用处理器架构处理请求
    async fn process_request_with_handler(&mut self, request: RemoteRequest) -> RemoteResponse {
        info!("Processing request with handler: {}", request.request_id);

        // Try to get a specific handler for this request type
        if let Some(handler) = HandlerFactory::create_handler(&request) {
            // Use the specific handler
            handler
                .handle(request, &mut self.chat, &self.config, &mut self.ws_stream)
                .await
        } else {
            // Use the chat handler for general chat requests
            let chat_handler = HandlerFactory::chat_handler();
            chat_handler
                .handle(request, &mut self.chat, &self.config, &mut self.ws_stream)
                .await
        }
    }

    /// 处理中断请求。
    fn handle_interrupt(&mut self, request_id: &str) -> RemoteResponse {
        info!("Handling interrupt request: {}", request_id);

        if self.chat.is_running() {
            self.chat.get_cancel_token().cancel();
            info!("Chat interrupted successfully");
            return RemoteResponse {
                request_id: request_id.to_string(),
                response: super::protocol::ResponseContent::Text(
                    "Model output interrupted successfully".to_string(),
                ),
                error: None,
                token_usage: None,
            };
        } else {
            return RemoteResponse::error(request_id, "No active model output to interrupt");
        }
    }
}
