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

## 版本历史

- v1.0.0 (初始版本): 支持基本文本对话和工具调用
- v1.1.0: 添加流式响应支持
- v1.2.0: 添加多种输入类型支持（图像、文件、指令等）
- v1.3.0: 协议从 TCP 迁移到 WebSocket，提供更好的双向通信支持

## 支持与反馈

如有问题或建议，请访问项目仓库提交 Issue。
