# Remote 模块通讯协议

## 概述

Agent CLI 的 Remote 模块提供了一个 WebSocket 服务器，允许外部客户端通过 JSON 协议与 AI 模型进行交互。该协议支持多种输入类型（文本、图像、指令、文件等）和配置选项。

## 快速开始

### 启动远程服务器

```bash
agent-cli --remote 127.0.0.1:8080
```

### 客户端连接示例 (Python)

```python
import asyncio
import websockets
import json

async def send_request(uri='ws://127.0.0.1:8080', request_data):
    async with websockets.connect(uri) as websocket:
        # 发送请求
        request_json = json.dumps(request_data)
        await websocket.send(request_json)
        
        # 接收响应
        response_data = await websocket.recv()
        return json.loads(response_data)

# 示例请求
request = {
    "request_id": "test_001",
    "input": {
        "Text": "你好，请介绍一下这个项目"
    },
    "stream": false,
    "use_tools": true
}

# 运行异步函数
response = asyncio.run(send_request('ws://127.0.0.1:8080', request))
print(response)
```

## 协议规范

### 消息格式

所有消息都使用 JSON 格式，通过 WebSocket 的 Text 消息类型传输。

### 请求结构 (RemoteRequest)

```json
{
  "request_id": "string",          // 请求唯一标识符
  "input": InputType,              // 输入数据
  "config": RequestConfig,         // 可选配置覆盖
  "stream": boolean,               // 是否流式响应（可选）
  "use_tools": boolean             // 是否使用工具（可选）
}
```

### 输入类型 (InputType)

支持多种输入类型：

#### 1. 文本输入
```json
{
  "Text": "要处理的文本内容"
}
```

#### 2. 图像输入
```json
{
  "Image": {
    "data": "base64编码的图像数据",
    "mime_type": "image/png"  // 可选
  }
}
```

#### 3. 指令输入
```json
{
  "Instruction": {
    "command": "指令名称",
    "parameters": {
      // 任意JSON参数
    }
  }
}
```

#### 4. 文件输入
```json
{
  "File": {
    "filename": "文件名",
    "content_type": "text/plain",
    "data": "base64编码的文件内容"
  }
}
```

#### 5. 复合输入
```json
{
  "Multi": [
    // 多个InputType对象
  ]
}
```

#### 6. 获取内置指令列表
```json
{
  "GetCommands": null
}
```
或简写形式：
```json
"GetCommands"
```

此输入类型用于请求获取Agent CLI的所有内置指令（TUI中的斜杠命令）列表。响应将包含命令名称和描述的JSON数组。

#### 7. 中断模型输出
```json
{
  "Interrupt": null
}
```
或简写形式：
```json
"Interrupt"
```

此输入类型用于中断当前正在进行的模型输出生成。如果模型正在生成响应，此命令将立即停止生成过程。

#### 8. 重新生成回复
```json
{
  "Regenerate": null
}
```
或简写形式：
```json
"Regenerate"
```

此输入类型用于重新生成最后的回复。如果模型已经完成了一次回复，此命令将使用相同的上下文重新生成回复，可用于当用户对之前的回复不满意时。

### 请求配置 (RequestConfig)

```json
{
  "max_tool_try": number,           // 最大工具尝试次数
  "max_context_num": number,        // 最大上下文数量
  "max_tokens": number,             // 最大token数
  "ask_before_tool_execution": boolean,  // 工具执行前是否询问
  "prompt": "string"                // 自定义提示词
}
```

### 响应结构 (RemoteResponse)

```json
{
  "request_id": "string",          // 对应的请求ID
  "response": ResponseContent,     // 响应内容
  "error": "string",               // 错误信息（可选）
  "token_usage": TokenUsage        // token使用统计（可选）
}
```

### 响应内容 (ResponseContent)

#### 1. 文本响应
```json
{
  "Text": "响应文本内容"
}
```

#### 2. 流式响应
```json
{
  "Stream": ["chunk1", "chunk2", "..."]
}
```

#### 3. 工具调用
```json
{
  "ToolCall": {
    "name": "工具名称",
    "arguments": {
      // 工具参数
    }
  }
}
```

#### 4. 工具结果
```json
{
  "ToolResult": {
    "name": "工具名称",
    "result": {
      // 工具执行结果
    }
  }
}
```

#### 5. 复合响应
```json
{
  "Multi": [
    // 多个ResponseContent对象
  ]
}
```

### Token使用统计 (TokenUsage)

```json
{
  "prompt_tokens": number,
  "completion_tokens": number,
  "total_tokens": number
}
```

## 使用示例

### 示例 1: 基本文本对话

**请求:**
```json
{
  "request_id": "chat_001",
  "input": {
    "Text": "你好，请介绍一下 Rust 语言的特点"
  },
  "stream": false,
  "use_tools": true
}
```

**响应:**
```json
{
  "request_id": "chat_001",
  "response": {
    "Text": "Rust 是一种系统编程语言，具有以下主要特点：\n1. 内存安全：通过所有权系统保证内存安全\n2. 零成本抽象：高级特性不带来运行时开销\n3. 并发安全：防止数据竞争\n4. 高性能：接近C/C++的性能\n..."
  },
  "error": null,
  "token_usage": {
    "prompt_tokens": 25,
    "completion_tokens": 120,
    "total_tokens": 145
  }
}
```

### 示例 2: 流式响应

**请求:**
```json
{
  "request_id": "stream_001",
  "input": {
    "Text": "写一个简单的 Rust Hello World 程序"
  },
  "stream": true,
  "use_tools": false
}
```

**响应:**
```json
{
  "request_id": "stream_001",
  "response": {
    "Stream": [
      "fn",
      " main",
      "()",
      " {",
      "\n    ",
      "println",
      "!",
      "(\"",
      "Hello",
      ", ",
      "World",
      "!",
      "\")",
      ";",
      "\n",
      "}"
    ]
  },
  "error": null,
  "token_usage": {
    "prompt_tokens": 15,
    "completion_tokens": 45,
    "total_tokens": 60
  }
}
```

### 示例 3: 带配置的请求

**请求:**
```json
{
  "request_id": "config_001",
  "input": {
    "Text": "分析这个项目的代码结构"
  },
  "config": {
    "max_tokens": 1000,
    "ask_before_tool_execution": false,
    "prompt": "你是一个代码分析专家，请详细分析代码结构"
  },
  "stream": false,
  "use_tools": true
}
```

### 示例 4: 获取内置指令列表

**请求:**
```json
{
  "request_id": "commands_001",
  "input": "GetCommands",
  "stream": false,
  "use_tools": false
}
```

**响应:**
```json
{
  "request_id": "commands_001",
  "response": {
    "Text": "{\"commands\":[{\"name\":\"help\",\"description\":\"显示帮助信息\"},{\"name\":\"clear\",\"description\":\"清除聊天记录\"},{\"name\":\"exit\",\"description\":\"退出程序\"},{\"name\":\"reset\",\"description\":\"重置对话上下文\"},{\"name\":\"history\",\"description\":\"显示历史记录\"},{\"name\":\"tools\",\"description\":\"显示可用工具列表\"},{\"name\":\"config\",\"description\":\"显示或修改配置\"}],\"count\":7}"
  },
  "error": null,
  "token_usage": {
    "prompt_tokens": 5,
    "completion_tokens": 10,
    "total_tokens": 15
  }
}
```

**说明:**
此请求用于获取Agent CLI的所有内置指令（TUI中的斜杠命令）列表。响应中的`response.Text`字段包含一个JSON字符串，其中`commands`数组包含每个命令的名称和描述，`count`字段表示命令总数。

要解析响应中的命令列表，客户端可以：
```python
import json

# 假设response是收到的RemoteResponse对象
response_text = response["response"]["Text"]
commands_data = json.loads(response_text)
commands = commands_data["commands"]
count = commands_data["count"]

for cmd in commands:
    print(f"命令: {cmd['name']}")
    print(f"描述: {cmd['description']}")
    print()
```

## 错误处理

### 错误响应示例

```json
{
  "request_id": "error_001",
  "response": {
    "Text": ""
  },
  "error": "Failed to parse request: expected value at line 1 column 1",
  "token_usage": null
}
```

### 常见错误码

- `parse_error`: 请求JSON解析失败
- `processing_error`: 处理请求时发生错误
- `connection_error`: 连接错误
- `timeout_error`: 请求超时

## 客户端实现指南

### Python 客户端

```python
import asyncio
import websockets
import json

class AgentCLIClient:
    def __init__(self, uri='ws://127.0.0.1:8080'):
        self.uri = uri
        self.websocket = None
        
    async def connect(self):
        self.websocket = await websockets.connect(self.uri)
        
    async def send_request(self, request_data):
        if not self.websocket:
            await self.connect()
            
        request_json = json.dumps(request_data)
        await self.websocket.send(request_json)
        
        # 接收响应
        response_data = await self.websocket.recv()
        return json.loads(response_data)
    
    async def close(self):
        if self.websocket:
            await self.websocket.close()
            self.websocket = None

# 使用示例
async def main():
    client = AgentCLIClient('ws://127.0.0.1:8080')
    try:
        response = await client.send_request({
            "request_id": "test_001",
            "input": {"Text": "你好"},
            "stream": False,
            "use_tools": True
        })
        print(response)
    finally:
        await client.close()

# 运行异步函数
asyncio.run(main())
```

### JavaScript/Node.js 客户端

```javascript
const WebSocket = require('ws');

class AgentCLIClient {
    constructor(uri = 'ws://127.0.0.1:8080') {
        this.uri = uri;
        this.ws = null;
    }

    connect() {
        return new Promise((resolve, reject) => {
            this.ws = new WebSocket(this.uri);
            
            this.ws.on('open', () => {
                resolve();
            });

            this.ws.on('error', (err) => {
                reject(err);
            });
        });
    }

    sendRequest(requestData) {
        return new Promise((resolve, reject) => {
            if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
                reject(new Error('Not connected'));
                return;
            }

            const requestJson = JSON.stringify(requestData);
            this.ws.send(requestJson);

            this.ws.once('message', (data) => {
                try {
                    const response = JSON.parse(data.toString());
                    resolve(response);
                } catch (err) {
                    reject(err);
                }
            });

            this.ws.once('error', (err) => {
                reject(err);
            });
        });
    }

    close() {
        if (this.ws) {
            this.ws.close();
            this.ws = null;
        }
    }
}

// 使用示例
async function main() {
    const client = new AgentCLIClient('ws://127.0.0.1:8080');
    try {
        await client.connect();
        const response = await client.sendRequest({
            request_id: 'test_001',
            input: { Text: 'Hello' },
            stream: false,
            use_tools: true
        });
        console.log(response);
    } catch (err) {
        console.error('Error:', err);
    } finally {
        client.close();
    }
}

main();
```

## 性能建议

1. **连接复用**: WebSocket 连接是持久化的，可以复用同一个连接处理多个请求
2. **心跳机制**: 实现 WebSocket ping/pong 心跳机制保持连接活跃
3. **批量处理**: 对于大量小请求，考虑批量处理
4. **超时设置**: 客户端应设置合理的连接和消息超时时间
5. **错误重试**: 实现适当的错误重试和连接重连机制
6. **资源清理**: 确保及时关闭连接释放资源

## 安全考虑

1. **网络隔离**: 建议在受信任的网络环境中使用
2. **访问控制**: 可通过防火墙规则限制访问IP
3. **数据加密**: 敏感数据建议在传输前进行加密
4. **输入验证**: 客户端应对输入数据进行验证

## 工具确认协议

### 工具确认请求

当配置中设置了 `ask_before_tool_execution: true` 时，服务器会在执行工具调用前向客户端发送工具确认请求。客户端需要响应此请求以确认或拒绝工具调用。

#### 工具确认请求格式

```json
{
  "request_id": "string",
  "response": {
    "ToolConfirmationRequest": {
      "name": "工具名称",
      "arguments": {
        // 工具参数
      },
      "description": "可选工具描述"
    }
  },
  "error": null,
  "token_usage": null
}
```

### 工具确认响应

客户端需要发送工具确认响应来批准或拒绝工具调用。

#### 工具确认响应格式

**请求格式:**
```json
{
  "request_id": "string",
  "input": {
    "ToolConfirmationResponse": {
      "name": "工具名称",
      "arguments": {
        // 工具参数（应与请求中的参数匹配）
      },
      "approved": true,
      "reason": "可选原因说明"
    }
  },
  "stream": false,
  "use_tools": true
}
```

**参数说明:**
- `name`: 工具名称，应与请求中的工具名称匹配
- `arguments`: 工具参数，应与请求中的参数匹配
- `approved`: 布尔值，true表示批准执行，false表示拒绝执行
- `reason`: 可选字符串，提供批准或拒绝的原因

### 错误信息增强

#### 结构化错误响应

工具执行错误现在包含更详细的结构化信息：

```json
{
  "request_id": "string",
  "response": {
    "Text": ""
  },
  "error": "{\"type\":\"tool_execution_error\",\"message\":\"Tool 'tool_name' execution failed\",\"details\":{\"tool\":\"tool_name\",\"error\":\"具体错误信息\",\"arguments\":{\"param1\":\"value1\"}}}",
  "token_usage": null
}
```

#### 工具错误响应方法

服务器现在提供 `RemoteResponse::tool_error()` 方法创建工具错误响应，包含：
- 错误类型: `tool_execution_error`
- 错误消息: 描述性错误信息
- 详细信息: 包含工具名称、具体错误信息和工具参数

## 版本历史

- v1.0.0 (初始版本): 支持基本文本对话和工具调用
- v1.1.0: 添加流式响应支持
- v1.2.0: 添加多种输入类型支持（图像、文件、指令等）
- v1.3.0: 协议从 TCP 迁移到 WebSocket，提供更好的双向通信支持
- v1.4.0: 添加获取内置指令列表功能（GetCommands），允许远端客户端查询TUI斜杠命令
- v1.5.0: 添加工具确认协议和增强的错误信息传递

## 支持与反馈

如有问题或建议，请访问项目仓库提交 Issue。
