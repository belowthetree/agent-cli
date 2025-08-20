use futures::Stream;

use crate::connection::CommonConnectionContent;

pub mod deepseek;
pub mod param;

pub trait AgentModel {
    async fn chat(&self, param: param::ModelInputParam) -> Result<Vec<CommonConnectionContent>, anyhow::Error>;
    async fn stream_chat(
        &self,
        param: param::ModelInputParam,
    ) -> impl Stream<Item = Result<CommonConnectionContent, anyhow::Error>>;
}
