//! ACP (Agent Client Protocol) 模块
//! 
//! 实现基于JSON-RPC 2.0的Agent Client Protocol
//! 支持stdio、WebSocket、HTTP三种传输方式

pub mod server;
pub mod transport;
pub mod handler;
pub mod types;
