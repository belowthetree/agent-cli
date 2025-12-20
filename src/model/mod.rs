///! # model
/// model 模块负责与模型沟通并将消息包装成模型要求的格式

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

    // 返回模型的上下文窗口大小（最大token数）
    async fn get_token_limit(&self) -> Result<u32, anyhow::Error>;
}