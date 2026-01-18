//! 远程指令模块
//!
//! 定义远程客户端可以执行的指令及其处理器。

use crate::chat::Chat;
use async_trait::async_trait;
use serde_json::Value;
use std::fmt::Debug;

/// 远程指令 trait
///
/// 所有远程指令都需要实现这个 trait
#[async_trait]
pub trait RemoteCommand: Send + Sync + Debug {
    /// 指令名称
    fn name(&self) -> &'static str;

    /// 指令描述
    fn description(&self) -> &'static str;

    /// 执行指令
    ///
    /// # 参数
    /// - `chat`: 聊天实例，用于访问上下文和执行操作
    /// - `parameters`: 指令参数（JSON格式）
    ///
    /// # 返回值
    /// - `Ok(String)`: 指令执行结果
    /// - `Err(String)`: 指令执行错误
    async fn execute(&self, chat: &mut Chat, parameters: Value) -> Result<String, String>;
}

/// 指令注册器
///
/// 用于注册和管理所有远程指令
pub struct CommandRegistry {
    commands: Vec<Box<dyn RemoteCommand>>,
}

impl CommandRegistry {
    /// 创建新的指令注册器
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    /// 注册指令
    #[allow(dead_code)]
    pub fn register(&mut self, command: Box<dyn RemoteCommand>) {
        self.commands.push(command);
    }

    /// 根据名称查找指令
    pub fn find(&self, name: &str) -> Option<&Box<dyn RemoteCommand>> {
        self.commands.iter().find(|cmd| cmd.name() == name)
    }

    /// 获取所有指令
    pub fn all(&self) -> &[Box<dyn RemoteCommand>] {
        &self.commands
    }
}

/// 全局指令注册器实例
use std::sync::OnceLock;

static COMMAND_REGISTRY: OnceLock<CommandRegistry> = OnceLock::new();

/// 初始化全局指令注册器
pub fn init_global_registry() -> &'static CommandRegistry {
    COMMAND_REGISTRY.get_or_init(|| {
        let registry = CommandRegistry::new();

        // 注册默认指令
        // 注意：clear_context 指令已移除，现在通过 ClearContext 协议变体实现

        registry
    })
}

/// 获取全局指令注册器
pub fn global_registry() -> &'static CommandRegistry {
    COMMAND_REGISTRY
        .get()
        .expect("Command registry not initialized")
}

// ========== 具体指令实现 ==========

// 注意：ClearContextCommand 已移除，现在通过 InputType::ClearContext 协议变体实现
