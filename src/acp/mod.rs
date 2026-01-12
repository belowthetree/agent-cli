//! ACP (Agent Client Protocol) 模块
//! 
//! 使用 agent-client-protocol 库实现 Agent Client Protocol
//! 支持 stdio 传输方式

pub mod agent_impl;

// 重新导出以保持向后兼容
#[allow(unused_imports)]
pub use agent_impl::{AcpAgent, run_stdio_agent};
