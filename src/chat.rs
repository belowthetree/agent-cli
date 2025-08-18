use crate::client::chat_client::{ChatClient, ChatResult, StreamedChatResponse};
use crate::config::Config;
use futures::Stream;
use rig::message::{AssistantContent, Message, Text};
use rig::OneOrMany;

pub struct Chat {
    pub client: ChatClient,
    pub context: Vec<Message>,
}

impl Chat {
    pub fn new(config: Config) -> Self {
        Self {
            client: ChatClient::new(config.deepseek_key),
            context: vec![],
        }
    }

    pub async fn chat(&mut self, prompt: &str) -> anyhow::Result<ChatResult> {
        let resp = self.client.chat(prompt, self.context.clone()).await;
        self.context.push(Message::Assistant { id: None, content: OneOrMany::one(AssistantContent::Text(Text{text: resp.content.clone()})) });
        Ok(resp)
    }

    pub fn stream_chat(
        &mut self,
        prompt: &str,
    ) -> impl Stream<Item = Result<StreamedChatResponse, anyhow::Error>> + '_ {
        self.client
            .stream_chat(prompt, self.context.clone())
    }
}