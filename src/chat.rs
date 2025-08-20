use futures::Stream;

use crate::client::chat_client::{ChatClient, ChatResult, StreamedChatResponse};
use crate::config::Config;
use crate::mcp::McpManager;
use crate::model::param::ModelMessage;

pub struct Chat {
    pub client: ChatClient,
    pub context: Vec<ModelMessage>,
}

impl Chat {
    pub fn new(config: Config) -> Self {
        Self {
            client: ChatClient::new(config.deepseek_key, McpManager::global().get_all_tools()),
            context: vec![],
        }
    }

    pub async fn chat(&mut self, prompt: &str) -> anyhow::Result<ChatResult> {
        let resp = self.client.chat(prompt, self.context.clone()).await;
        let t = resp.clone();
        self.context.push(ModelMessage::assistant(resp.content, resp.think, resp.tool_calls));
        Ok(t)
    }

    pub fn stream_chat(
        &mut self,
        prompt: &str,
    ) -> impl Stream<Item = Result<StreamedChatResponse, anyhow::Error>> + '_ {
        self.client
            .stream_chat(prompt, self.context.clone())
    }
}