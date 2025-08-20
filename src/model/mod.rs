pub mod deepseek;
pub mod param;

pub trait AgentModel {
    async fn chat(&self, param: param::ModelInputParam) -> Result<param::ModelResponse, String>;
    async fn stream_chat(
        &self,
        param: param::ModelInputParam,
    ) -> Result<impl futures_util::Stream<Item = Result<param::ModelResponse, String>>, String>;
}
