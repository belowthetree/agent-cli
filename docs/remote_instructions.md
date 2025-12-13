# 远程指令系统

## 概述

远程指令系统允许远程客户端通过 WebSocket 连接执行预定义的指令。系统设计为可扩展，可以轻松添加新的指令。

## 架构

### 核心组件

1. **RemoteCommand Trait** (`src/remote/commands.rs`)
   - 定义所有远程指令必须实现的接口
   - 包含 `name()`, `description()`, `execute()` 方法

2. **CommandRegistry** (`src/remote/commands.rs`)
   - 管理所有已注册的指令
   - 提供指令查找功能

3. **全局注册器** (`src/remote/commands.rs`)
   - 使用 `OnceLock` 实现的单例模式
   - 在服务器启动时初始化

4. **指令处理器** (`src/remote/client_handler.rs`)
   - 处理 `InputType::Instruction` 类型的请求
   - 调用相应的指令执行器

### 数据流

```
客户端请求 → WebSocket → ClientHandler → 指令处理器 → RemoteCommand.execute() → 响应
```

## 内置指令

### clear_context

**描述**: 清理聊天上下文，重置对话轮次

**参数**: 无

**功能**:
1. 重置对话轮次计数器
2. 清理所有聊天消息
3. 保留系统消息（如果存在）

**响应**: "上下文已清理，对话轮次已重置"

## 使用方法

### 1. 启动远程服务器

```bash
cargo run -- remote start
```

### 2. 客户端请求格式

#### 获取指令列表

```json
{
  "request_id": "test_1",
  "input": {
    "type": "get_commands"
  }
}
```

#### 执行指令

```json
{
  "request_id": "test_2",
  "input": {
    "type": "instruction",
    "command": "clear_context",
    "parameters": {}
  }
}
```

### 3. Python 客户端示例

```python
import asyncio
import websockets
import json

async def execute_command():
    uri = "ws://localhost:8080"
    
    async with websockets.connect(uri) as websocket:
        # 获取指令列表
        request = {
            "request_id": "get_cmds",
            "input": {"type": "get_commands"}
        }
        await websocket.send(json.dumps(request))
        response = await websocket.recv()
        print("可用指令:", response)
        
        # 执行清理上下文指令
        request = {
            "request_id": "clear_ctx",
            "input": {
                "type": "instruction",
                "command": "clear_context",
                "parameters": {}
            }
        }
        await websocket.send(json.dumps(request))
        response = await websocket.recv()
        print("执行结果:", response)

asyncio.run(execute_command())
```

## 添加新指令

### 步骤 1: 创建指令实现

在 `src/remote/commands.rs` 中添加新的指令结构体：

```rust
#[derive(Debug)]
pub struct YourCommand;

#[async_trait]
impl RemoteCommand for YourCommand {
    fn name(&self) -> &'static str {
        "your_command"
    }
    
    fn description(&self) -> &'static str {
        "你的指令描述"
    }
    
    async fn execute(&self, chat: &mut Chat, parameters: Value) -> Result<String, String> {
        // 实现指令逻辑
        Ok("执行结果".to_string())
    }
}
```

### 步骤 2: 注册指令

在 `init_global_registry()` 函数中注册新指令：

```rust
pub fn init_global_registry() -> &'static CommandRegistry {
    COMMAND_REGISTRY.get_or_init(|| {
        let mut registry = CommandRegistry::new();
        
        // 注册默认指令
        registry.register(Box::new(ClearContextCommand));
        registry.register(Box::new(YourCommand)); // 添加这一行
        
        registry
    })
}
```

### 步骤 3: 添加测试

在 `src/remote/mod.rs` 的测试模块中添加测试：

```rust
#[tokio::test]
async fn test_your_command() {
    // 测试你的指令
}
```

## 协议扩展

### InputType 枚举

指令系统扩展了 `InputType` 枚举，添加了新的变体：

```rust
pub enum InputType {
    // ... 其他变体
    GetCommands,
    Instruction {
        command: String,
        parameters: serde_json::Value,
    },
}
```

### 错误处理

- 未知指令: 返回错误响应 "Unknown command: {command_name}"
- 执行失败: 返回错误响应 "Command execution failed: {error_message}"

## 测试

运行测试确保指令系统正常工作：

```bash
# 运行所有远程模块测试
cargo test remote

# 运行特定指令测试
cargo test test_instruction_system
```

## 最佳实践

1. **指令命名**: 使用蛇形命名法，如 `clear_context`
2. **参数验证**: 在 `execute()` 方法中验证参数
3. **错误信息**: 提供清晰的错误信息
4. **文档**: 为每个指令添加详细的文档
5. **测试**: 为每个指令编写单元测试

## 性能考虑

1. **异步执行**: 所有指令都是异步执行的
2. **内存管理**: 指令执行后及时释放资源
3. **并发安全**: 指令实现必须是 `Send + Sync`
