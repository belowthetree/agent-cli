use std::io::{self, Write};

use futures::{pin_mut, Stream, StreamExt};

use crate::chat::StreamedChatResponse;

pub mod chat_client;
pub mod tool_client;

pub async fn handle_output(stream: impl Stream<Item = Result<StreamedChatResponse, anyhow::Error>> + '_) {
    pin_mut!(stream);
    while let Some(result) = stream.next().await {
        if let Ok(res) = result {
            match res {
                StreamedChatResponse::Text(text) => print!("{}", text),
                StreamedChatResponse::ToolCall(tool_call) => print!("{:?}", tool_call),
                StreamedChatResponse::Reasoning(think) => print!("{}", think),
                StreamedChatResponse::ToolResponse(tool) => print!("{:?}", tool),
                StreamedChatResponse::End => {}
            }
            io::stdout().flush().unwrap();
        }
    }
}