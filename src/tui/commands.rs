use async_trait::async_trait;
use std::fmt::Debug;

/// TUI斜杠命令trait
/// 
/// 所有TUI斜杠命令都需要实现这个trait
#[async_trait]
pub trait TuiCommand: Send + Sync + Debug {
    /// 命令名称（不带斜杠）
    fn name(&self) -> &'static str;
    
    /// 命令描述
    fn description(&self) -> &'static str;
    
    /// 执行命令
    /// 
    /// # 参数
    /// - `app`: 应用程序引用，用于修改应用状态
    /// - `args`: 命令参数（如果有）
    /// 
    /// # 返回值
    /// - `true`: 命令执行成功
    /// - `false`: 命令执行失败或未找到
    async fn execute(&self, app: &mut crate::tui::app::App, args: &str) -> bool;
}

/// 命令注册器
/// 
/// 用于注册和管理所有TUI斜杠命令
pub struct CommandRegistry {
    commands: Vec<Box<dyn TuiCommand>>,
}

impl CommandRegistry {
    /// 创建新的命令注册器
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }
    
    /// 注册命令
    pub fn register(&mut self, command: Box<dyn TuiCommand>) {
        self.commands.push(command);
    }
    
    /// 根据名称查找命令
    pub fn find(&self, name: &str) -> Option<&Box<dyn TuiCommand>> {
        self.commands.iter().find(|cmd| cmd.name() == name)
    }
    
    /// 获取所有命令
    pub fn all(&self) -> &[Box<dyn TuiCommand>] {
        &self.commands
    }
    
    /// 获取所有命令名称（带斜杠）
    pub fn command_names(&self) -> Vec<String> {
        self.commands.iter()
            .map(|cmd| format!("/{}", cmd.name()))
            .collect()
    }
}

/// 全局命令注册器实例
use std::sync::OnceLock;

static COMMAND_REGISTRY: OnceLock<CommandRegistry> = OnceLock::new();

/// 初始化全局命令注册器
pub fn init_global_registry() -> &'static CommandRegistry {
    COMMAND_REGISTRY.get_or_init(|| {
        let mut registry = CommandRegistry::new();
        
        // 注册默认命令
        registry.register(Box::new(HelpCommand));
        registry.register(Box::new(ClearCommand));
        registry.register(Box::new(ExitCommand));
        registry.register(Box::new(ResetCommand));
        registry.register(Box::new(HistoryCommand));
        registry.register(Box::new(ToolsCommand));
        registry.register(Box::new(ConfigCommand));
        
        registry
    })
}

/// 获取全局命令注册器
pub fn global_registry() -> &'static CommandRegistry {
    COMMAND_REGISTRY.get().expect("Command registry not initialized")
}

// ========== 具体命令实现 ==========

/// 帮助命令
#[derive(Debug)]
pub struct HelpCommand;

#[async_trait]
impl TuiCommand for HelpCommand {
    fn name(&self) -> &'static str {
        "help"
    }
    
    fn description(&self) -> &'static str {
        "显示帮助信息"
    }
    
    async fn execute(&self, app: &mut crate::tui::app::App, _args: &str) -> bool {
        let registry = global_registry();
        let mut help_text = String::from("可用命令:\n");
        
        for cmd in registry.all() {
            help_text.push_str(&format!("  /{} - {}\n", cmd.name(), cmd.description()));
        }
        
        app.add_info_message(&help_text);
        true
    }
}

/// 清除命令
#[derive(Debug)]
pub struct ClearCommand;

#[async_trait]
impl TuiCommand for ClearCommand {
    fn name(&self) -> &'static str {
        "clear"
    }
    
    fn description(&self) -> &'static str {
        "清除聊天记录"
    }
    
    async fn execute(&self, app: &mut crate::tui::app::App, _args: &str) -> bool {
        app.blocks.clear();
        app.info_messages.clear();
        app.max_line = 0;
        app.index = 0;
        app.add_system_message("聊天记录和信息消息已清除");
        true
    }
}

/// 退出命令
#[derive(Debug)]
pub struct ExitCommand;

#[async_trait]
impl TuiCommand for ExitCommand {
    fn name(&self) -> &'static str {
        "exit"
    }
    
    fn description(&self) -> &'static str {
        "退出程序"
    }
    
    async fn execute(&self, app: &mut crate::tui::app::App, _args: &str) -> bool {
        app.should_exit.store(true, std::sync::atomic::Ordering::Relaxed);
        true
    }
}

/// 重置命令
#[derive(Debug)]
pub struct ResetCommand;

#[async_trait]
impl TuiCommand for ResetCommand {
    fn name(&self) -> &'static str {
        "reset"
    }
    
    fn description(&self) -> &'static str {
        "重置对话"
    }
    
    async fn execute(&self, app: &mut crate::tui::app::App, _args: &str) -> bool {
        {
            let mut chat = app.chat.lock().unwrap();
            chat.reset_conversation_turn();
        }
        app.add_system_message("对话轮次已重置");
        true
    }
}

/// 历史命令
#[derive(Debug)]
pub struct HistoryCommand;

#[async_trait]
impl TuiCommand for HistoryCommand {
    fn name(&self) -> &'static str {
        "history"
    }
    
    fn description(&self) -> &'static str {
        "显示历史记录"
    }
    
    async fn execute(&self, app: &mut crate::tui::app::App, _args: &str) -> bool {
        let history = {
            let chat = app.chat.lock().unwrap();
            chat.context().iter()
                .filter(|msg| msg.role == "user" || msg.role == "assistant")
                .map(|msg| format!("[{}] {}", msg.role, msg.content))
                .collect::<Vec<String>>()
        };
        app.add_system_message(&format!("历史记录 ({} 条):\n{}", history.len(), history.join("\n")));
        true
    }
}

/// 工具命令
#[derive(Debug)]
pub struct ToolsCommand;

#[async_trait]
impl TuiCommand for ToolsCommand {
    fn name(&self) -> &'static str {
        "tools"
    }
    
    fn description(&self) -> &'static str {
        "显示可用工具"
    }
    
    async fn execute(&self, app: &mut crate::tui::app::App, _args: &str) -> bool {
        // 由于tools字段是私有的，我们无法直接访问
        // 暂时显示一个通用消息
        app.add_system_message("工具信息: 无法直接访问工具列表（私有字段）");
        true
    }
}

/// 配置命令
#[derive(Debug)]
pub struct ConfigCommand;

#[async_trait]
impl TuiCommand for ConfigCommand {
    fn name(&self) -> &'static str {
        "config"
    }
    
    fn description(&self) -> &'static str {
        "显示配置信息"
    }
    
    async fn execute(&self, app: &mut crate::tui::app::App, _args: &str) -> bool {
        // 由于这些字段是私有的，我们无法直接访问
        // 暂时显示一个通用消息
        app.add_system_message("配置信息: 无法直接访问配置字段（私有字段）");
        true
    }
}
