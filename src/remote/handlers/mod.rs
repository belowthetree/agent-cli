//! 请求处理器模块

mod base_handler;
mod chat_handler;
mod command_handler;
mod instruction_handler;
mod interrupt_handler;
mod regenerate_handler;
mod clear_context_handler;
mod tool_confirmation_handler;

pub use base_handler::RequestHandler;
pub use chat_handler::ChatHandler;
pub use command_handler::CommandHandler;
pub use instruction_handler::InstructionHandler;
pub use interrupt_handler::InterruptHandler;
pub use regenerate_handler::RegenerateHandler;
pub use clear_context_handler::ClearContextHandler;
pub use tool_confirmation_handler::ToolConfirmationHandler;

use crate::remote::protocol::RemoteRequest;

/// 处理器工厂，根据请求类型创建相应的处理器
pub struct HandlerFactory;

impl HandlerFactory {
    /// 根据请求类型创建相应的处理器
    pub fn create_handler(request: &RemoteRequest) -> Option<Box<dyn RequestHandler>> {
        use crate::remote::protocol::InputType;
        
        match &request.input {
            InputType::GetCommands => Some(Box::new(CommandHandler)),
            InputType::Instruction { command: _, parameters: _ } => Some(Box::new(InstructionHandler)),
            InputType::Interrupt => Some(Box::new(InterruptHandler)),
            InputType::Regenerate => Some(Box::new(RegenerateHandler)),
            InputType::ClearContext => Some(Box::new(ClearContextHandler)),
            InputType::ToolConfirmationResponse { name: _, arguments: _, approved: _, reason: _ } => 
                Some(Box::new(ToolConfirmationHandler)),
            _ => None, // 普通聊天请求由 ChatHandler 处理
        }
    }
    
    /// 获取聊天处理器（用于普通聊天请求）
    pub fn chat_handler() -> Box<dyn RequestHandler> {
        Box::new(ChatHandler)
    }
}
