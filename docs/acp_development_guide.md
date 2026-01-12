# ACP 协议开发指南

> Agent Client Protocol (ACP) 开发者文档
> 版本：0.1.0
> 最后更新：2026-01-12

---

## 目录

1. [快速开始](#快速开始)
2. [协议概述](#协议概述)
3. [核心概念](#核心概念)
4. [协议规范](#协议规范)
5. [实现指南](#实现指南)
6. [最佳实践](#最佳实践)
7. [安全考量](#安全考量)
8. [API 参考](#api-参考)
9. [常见问题](#常见问题)

---

## 快速开始

### 什么是 ACP？

**Agent Client Protocol (ACP)** 是一个开放标准协议，用于规范代码编辑器与 AI 编码助手（Coding Agent）之间的通信。

### 核心价值

- **通用性**：任何编辑器都能集成任何符合 ACP 的 Agent
- **隐私优先**：本地通信，不经过第三方服务器
- **开源开放**：Apache 2.0 许可证
- **可扩展性**：支持未来的新功能和新场景

### 与 LSP 的类比

| LSP | ACP |
|-----|-----|
| Language Server Protocol | Agent Client Protocol |
| 语言智能 ↔ IDE | AI 编码助手 ↔ 代码编辑器 |
| 编辑器 ↔ 语言服务器 | 编辑器 ↔ AI Agent |

### 30 秒快速上手

```typescript
// 1. 启动 Agent 进程
const agentProcess = spawn('claude-code', [], {
  cwd: '/workspace',
  stdio: ['pipe', 'pipe', 'pipe'],
});

// 2. 发送初始化请求
const initializeRequest = {
  jsonrpc: "2.0",
  id: 1,
  method: "initialize",
  params: {
    protocolVersion: "0.1.0",
    clientInfo: { name: "MyEditor", version: "1.0.0" }
  }
};
agentProcess.stdin.write(JSON.stringify(initializeRequest) + '\n');

// 3. 接收响应
agentProcess.stdout.on('data', (data) => {
  const response = JSON.parse(data.toString().trim());
  console.log('Agent capabilities:', response.result.capabilities);
});
```

---

## 协议概述

### 通信模型

ACP 采用 **JSON-RPC 2.0** 协议，基于 **stdio（标准输入输出）** 进行通信。

```
编辑器                    Agent
  |                        |
  |-------- initialize -->|
  |<-- capabilities -------|
  |                        |
  |------ session/new --->|
  |<--- sessionId ---------|
  |                        |
  |---- session/prompt -->|
  |                        |
  |<- session/update -----|  (流式更新)
  |<- session/update -----|
  |<-- end_turn ----------|
  |                        |
```

### 通信方式

| 特性 | 说明 |
|-----|------|
| **协议基础** | JSON-RPC 2.0 |
| **传输层** | stdio (标准输入输出) |
| **消息格式** | JSON，每行一条消息 |
| **消息类型** | Request / Response / Notification |
| **连接方式** | 编辑器启动 Agent 子进程 |

### 优势

- ✅ **简单高效**：无需网络层，直接进程间通信
- ✅ **隐私安全**：所有数据都在本地，不经过外部服务器
- ✅ **跨平台**：stdin/stdout 是所有操作系统的标准
- ✅ **易于调试**：可以手动输入 JSON 消息测试

---

## 核心概念

### 消息类型

ACP 支持三种消息类型：

#### 1. Request（请求）

客户端向服务器发送请求，期待响应。

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "protocolVersion": "0.1.0",
    "clientInfo": {
      "name": "Zed",
      "version": "0.158.0"
    }
  }
}
```

#### 2. Response（响应）

服务器对请求的响应。

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "protocolVersion": "0.1.0",
    "serverInfo": {
      "name": "Claude Code",
      "version": "1.0.0"
    },
    "capabilities": {
      "tools": true,
      "resources": true
    }
  }
}
```

#### 3. Notification（通知）

单向消息，不期待响应。

```json
{
  "jsonrpc": "2.0",
  "method": "session/update",
  "params": {
    "sessionId": "session-123",
    "update": {
      "sessionUpdate": "agent_message_chunk",
      "content": {
        "type": "text",
        "text": "正在分析代码..."
      }
    }
  }
}
```

### 会话更新类型

ACP 定义了丰富的会话更新类型，让编辑器能实时显示 Agent 的思考和操作过程。

| 更新类型 | 说明 | 用途 |
|---------|------|------|
| `agent_message_chunk` | Agent 消息块 | 显示 AI 的回复内容 |
| `agent_thought_chunk` | Agent 思考过程 | 显示 AI 的内部推理 |
| `tool_call` | 工具调用 | 显示正在执行的操作 |
| `plan` | 任务计划 | 显示执行步骤列表 |
| `available_commands` | 可用命令 | 显示 Agent 提供的命令 |
| `user_message_chunk` | 用户消息块 | 显示用户输入确认 |

### 权限请求机制

ACP 内置权限请求机制，Agent 在执行敏感操作前必须获得用户许可。

```typescript
interface PermissionRequest {
  sessionId: string;
  options: Array<{
    optionId: string;
    name: string;
    kind: 'allow_once' | 'allow_always' | 'reject_once' | 'reject_always';
  }>;
  toolCall: {
    toolCallId: string;
    title: string;
    kind: 'read' | 'edit' | 'execute';
    content?: Array<any>;
    locations?: Array<{ path: string }>;
  };
}
```

---

## 协议规范

### 核心方法

#### initialize

初始化连接，交换能力信息。

**请求：**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "protocolVersion": "0.1.0",
    "clientInfo": {
      "name": "MyEditor",
      "version": "1.0.0"
    }
  }
}
```

**响应：**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "protocolVersion": "0.1.0",
    "serverInfo": {
      "name": "Claude Code",
      "version": "1.0.0"
    },
    "capabilities": {
      "tools": true,
      "resources": true,
      "streaming": false
    }
  }
}
```

#### session/new

创建新的对话会话。

**请求：**
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "session/new",
  "params": {
    "cwd": "/workspace",
    "mcpServers": []
  }
}
```

**响应：**
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "sessionId": "session-123"
  }
}
```

#### session/prompt

向 Agent 发送用户消息。

**请求：**
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "session/prompt",
  "params": {
    "prompt": "帮我重构这个函数"
  }
}
```

**响应：**
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "success": true
  }
}
```

#### session/update (Notification)

Agent 推送会话更新（流式）。

```json
{
  "jsonrpc": "2.0",
  "method": "session/update",
  "params": {
    "sessionId": "session-123",
    "update": {
      "sessionUpdate": "agent_message_chunk",
      "content": {
        "type": "text",
        "text": "我会帮你重构这个函数..."
      }
    }
  }
}
```

#### session/request_permission (Notification)

Agent 请求用户权限。

```json
{
  "jsonrpc": "2.0",
  "method": "session/request_permission",
  "params": {
    "sessionId": "session-123",
    "options": [
      {
        "optionId": "allow_once",
        "name": "仅此一次允许",
        "kind": "allow_once"
      },
      {
        "optionId": "allow_always",
        "name": "始终允许",
        "kind": "allow_always"
      },
      {
        "optionId": "reject",
        "name": "拒绝",
        "kind": "reject_once"
      }
    ],
    "toolCall": {
      "toolCallId": "tool-001",
      "title": "写入文件 src/index.ts",
      "kind": "edit",
      "locations": [{ "path": "/workspace/src/index.ts" }],
      "content": [
        {
          "type": "diff",
          "diff": "--- a/src/index.ts\n+++ b/src/index.ts\n@@ -10,7 +10,7 @@\n-function add(a, b) {\n+function add(a: number, b: number): number {\n   return a + b;\n }"
        }
      ]
    }
  }
}
```

#### end_turn (Notification)

Agent 完成一轮响应。

```json
{
  "jsonrpc": "2.0",
  "method": "end_turn",
  "params": {
    "sessionId": "session-123"
  }
}
```

---

## 实现指南

### 客户端实现（编辑器端）

#### 1. 启动 Agent 进程

```typescript
import { spawn } from 'child_process';

class AcpClient {
  private agentProcess: ChildProcess | null = null;
  private pendingRequests: Map<number, PendingRequest> = new Map();
  private requestIdCounter = 0;

  async connect(command: string, workingDir: string) {
    this.agentProcess = spawn(command, [], {
      cwd: workingDir,
      env: process.env,
      stdio: ['pipe', 'pipe', 'pipe'],
    });

    // 监听 stdout
    this.agentProcess.stdout.on('data', this.handleStdout.bind(this));

    // 监听 stderr（用于日志）
    this.agentProcess.stderr.on('data', this.handleStderr.bind(this));

    // 初始化协议
    await this.initialize();
  }
}
```

#### 2. 发送请求

```typescript
private async sendRequest(
  method: string,
  params: any,
  timeout: number = 60000
): Promise<any> {
  const id = ++this.requestIdCounter;

  const request = {
    jsonrpc: "2.0",
    id,
    method,
    params,
  };

  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      this.pendingRequests.delete(id);
      reject(new Error(`Request ${method} timeout after ${timeout}ms`));
    }, timeout);

    this.pendingRequests.set(id, { resolve, reject, timer });

    // 写入 Agent 的 stdin
    const message = JSON.stringify(request) + '\n';
    this.agentProcess.stdin.write(message);
  });
}
```

#### 3. 处理响应

```typescript
private handleStdout(data: Buffer) {
  const lines = data
    .toString()
    .split('\n')
    .filter((line) => line.trim());

  for (const line of lines) {
    try {
      const message = JSON.parse(line);

      if ('id' in message && 'result' in message) {
        // Response: 匹配请求并 resolve
        const pending = this.pendingRequests.get(message.id);
        if (pending) {
          clearTimeout(pending.timer);
          pending.resolve(message);
          this.pendingRequests.delete(message.id);
        }
      } else if ('method' in message) {
        // Notification: 触发回调
        this.handleNotification(message);
      }
    } catch (error) {
      console.error('Failed to parse message:', line, error);
    }
  }
}
```

#### 4. 处理通知

```typescript
private handleNotification(notification: any) {
  const { method, params } = notification;

  switch (method) {
    case 'session/update':
      this.onSessionUpdate?.(params.update);
      break;

    case 'session/request_permission':
      this.onPermissionRequest?.(params);
      break;

    case 'end_turn':
      this.onEndTurn?.();
      break;
  }
}
```

### 服务端实现（Agent 端）

#### 1. 读取请求

```typescript
import readline from 'readline';

const rl = readline.createInterface({
  input: process.stdin,
  output: process.stdout,
  terminal: false,
});

rl.on('line', (line: string) => {
  if (!line.trim()) return;

  try {
    const request = JSON.parse(line);
    handleRequest(request);
  } catch (error) {
    sendError(null, -32700, 'Parse error');
  }
});
```

#### 2. 发送响应

```typescript
function sendResponse(id: number, result: any) {
  const response = {
    jsonrpc: "2.0",
    id,
    result,
  };
  console.log(JSON.stringify(response));
}

function sendNotification(method: string, params: any) {
  const notification = {
    jsonrpc: "2.0",
    method,
    params,
  };
  console.log(JSON.stringify(notification));
}

function sendError(id: number | null, code: number, message: string, data?: any) {
  const error = {
    jsonrpc: "2.0",
    id,
    error: {
      code,
      message,
      data,
    },
  };
  console.log(JSON.stringify(error));
}
```

#### 3. 处理初始化

```typescript
async function handleInitialize(request: any) {
  const { protocolVersion, clientInfo } = request.params;

  // 验证协议版本
  if (protocolVersion !== "0.1.0") {
    sendError(request.id, -32602, 'Unsupported protocol version');
    return;
  }

  // 返回能力信息
  sendResponse(request.id, {
    protocolVersion: "0.1.0",
    serverInfo: {
      name: "MyAgent",
      version: "1.0.0",
    },
    capabilities: {
      tools: true,
      resources: true,
      streaming: true,
    },
  });
}
```

#### 4. 处理会话

```typescript
async function handleSessionNew(request: any) {
  const { cwd, mcpServers } = request.params;

  // 创建新会话
  const sessionId = generateSessionId();
  sessions.set(sessionId, { cwd, mcpServers });

  sendResponse(request.id, { sessionId });
}

async function handleSessionPrompt(request: any) {
  const { sessionId, prompt } = request.params;

  const session = sessions.get(sessionId);
  if (!session) {
    sendError(request.id, -32602, 'Invalid session ID');
    return;
  }

  // 异步处理请求
  processPrompt(sessionId, prompt);

  sendResponse(request.id, { success: true });
}
```

#### 5. 流式更新

```typescript
async function processPrompt(sessionId: string, prompt: string) {
  // 发送思考过程
  sendNotification('session/update', {
    sessionId,
    update: {
      sessionUpdate: 'agent_thought_chunk',
      content: {
        type: 'text',
        text: '正在分析代码...'
      },
    },
  });

  // 读取文件
  sendNotification('session/update', {
    sessionId,
    update: {
      sessionUpdate: 'tool_call',
      toolCallId: 'tool-001',
      status: 'pending',
      title: '读取 src/index.ts',
      kind: 'read',
      locations: [{ path: '/workspace/src/index.ts' }],
    },
  });

  // 执行工具调用...

  // 发送消息
  sendNotification('session/update', {
    sessionId,
    update: {
      sessionUpdate: 'agent_message_chunk',
      content: {
        type: 'text',
        text: '代码分析完成！'
      },
    },
  });

  // 结束本轮
  sendNotification('end_turn', { sessionId });
}
```

---

## 最佳实践

### 日志处理

❌ **错误做法：**

```typescript
// 不要写入 stdout！
console.log('Agent is processing...');
```

✅ **正确做法：**

```typescript
// 使用 stderr 或文件日志
import fs from 'fs';

const logFile = fs.createWriteStream('/tmp/agent.log');

function log(message: string) {
  logFile.write(`[${new Date().toISOString()}] ${message}\n`);
}

log('Agent is processing...');
```

### 错误处理

```typescript
try {
  await executeToolCall(toolCall);
} catch (error) {
  // 返回标准错误格式
  return {
    jsonrpc: "2.0",
    id: requestId,
    error: {
      code: -32603,
      message: error.message,
      data: {
        stack: error.stack,
      },
    },
  };
}
```

### 超时管理

```typescript
const TOOL_CALL_TIMEOUT = 30000; // 30 秒

async function executeToolCallWithTimeout(toolCall: ToolCall) {
  return Promise.race([
    executeToolCall(toolCall),
    new Promise((_, reject) => 
      setTimeout(() => reject(new Error('Tool call timeout')), TOOL_CALL_TIMEOUT)
    )
  ]);
}
```

### 批量操作

```typescript
async function readMultipleFiles(paths: string[]): Promise<Map<string, string>> {
  const results = new Map();

  await Promise.all(
    paths.map(async (path) => {
      const content = await fs.readFile(path, 'utf-8');
      results.set(path, content);
    })
  );

  return results;
}
```

### 缓存机制

```typescript
import LRU from 'lru-cache';

const fileCache = new LRU<string, string>({
  max: 100,
  maxAge: 5 * 60 * 1000, // 5 分钟
});

async function readFileWithCache(path: string): Promise<string> {
  const cached = fileCache.get(path);
  if (cached) return cached;

  const content = await fs.readFile(path, 'utf-8');
  fileCache.set(path, content);

  return content;
}
```

---

## 安全考量

### 文件系统访问控制

```typescript
const ALLOWED_WORKSPACE = process.env.WORKSPACE_DIR;

function validateFilePath(path: string): boolean {
  const resolvedPath = path.resolve(path);

  // 检查路径是否在允许的工作目录内
  if (!resolvedPath.startsWith(ALLOWED_WORKSPACE)) {
    throw new Error('Access denied: path outside workspace');
  }

  // 检查是否访问敏感文件
  const sensitivePatterns = ['.env', '.git/config', 'id_rsa'];
  if (sensitivePatterns.some((pattern) => resolvedPath.includes(pattern))) {
    throw new Error('Access denied: sensitive file');
  }

  return true;
}
```

### 命令执行安全

```typescript
const ALLOWED_COMMANDS = ['npm test', 'npm run lint', 'tsc --noEmit', 'git status'];

function validateCommand(command: string): boolean {
  return ALLOWED_COMMANDS.some((allowed) => command.startsWith(allowed));
}

async function executeCommand(command: string) {
  if (!validateCommand(command)) {
    throw new Error('Command not allowed');
  }

  return execAsync(command, {
    timeout: 30000,
    cwd: WORKSPACE_DIR,
  });
}
```

### 敏感信息处理

```typescript
function sanitizeContent(content: string): string {
  // 移除 API 密钥
  content = content.replace(/sk-[a-zA-Z0-9]{48}/g, '***API_KEY***');

  // 移除密码
  content = content.replace(/password\s*=\s*['"][^'"]+['"]/gi, 'password=***');

  // 移除 Token
  content = content.replace(/Bearer\s+[a-zA-Z0-9._-]+/g, 'Bearer ***');

  return content;
}
```

---

## API 参考

### 请求方法

#### initialize

初始化连接。

**参数：**
```typescript
interface InitializeParams {
  protocolVersion: string;
  clientInfo: {
    name: string;
    version: string;
  };
}
```

**返回：**
```typescript
interface InitializeResult {
  protocolVersion: string;
  serverInfo: {
    name: string;
    version: string;
  };
  capabilities: {
    tools?: boolean;
    resources?: boolean;
    streaming?: boolean;
  };
}
```

#### session/new

创建新会话。

**参数：**
```typescript
interface SessionNewParams {
  cwd: string;
  mcpServers?: Array<{
    name: string;
    command: string;
    args: string[];
  }>;
}
```

**返回：**
```typescript
interface SessionNewResult {
  sessionId: string;
}
```

#### session/prompt

发送用户消息。

**参数：**
```typescript
interface SessionPromptParams {
  prompt: string;
}
```

**返回：**
```typescript
interface SessionPromptResult {
  success: boolean;
}
```

### 通知方法

#### session/update

会话更新通知。

**参数：**
```typescript
interface SessionUpdateParams {
  sessionId: string;
  update: SessionUpdate;
}
```

#### session/request_permission

权限请求通知。

**参数：**
```typescript
interface PermissionRequestParams {
  sessionId: string;
  options: PermissionOption[];
  toolCall: ToolCall;
}
```

#### end_turn

结束本轮响应。

**参数：**
```typescript
interface EndTurnParams {
  sessionId: string;
}
```

### 类型定义

#### SessionUpdate

```typescript
type SessionUpdate =
  | AgentMessageChunkUpdate
  | AgentThoughtChunkUpdate
  | ToolCallUpdate
  | PlanUpdate
  | AvailableCommandsUpdate
  | UserMessageChunkUpdate;
```

#### ToolCallUpdate

```typescript
interface ToolCallUpdate {
  sessionUpdate: 'tool_call';
  toolCallId: string;
  status: 'pending' | 'in_progress' | 'completed' | 'failed';
  title: string;
  kind: 'read' | 'edit' | 'execute';
  rawInput?: any;
  content?: Array<{
    type: 'content' | 'diff';
    [key: string]: any;
  }>;
  locations?: Array<{
    path: string;
  }>;
}
```

#### PlanUpdate

```typescript
interface PlanUpdate {
  sessionUpdate: 'plan';
  entries: Array<{
    content: string;
    status: 'pending' | 'in_progress' | 'completed';
    priority?: 'high' | 'medium' | 'low';
  }>;
}
```

---

## 常见问题

### Q1: 为什么使用 stdio 而不是 HTTP？

**A:** stdio 有以下优势：
- 简单高效，无需网络层
- 隐私安全，所有数据在本地
- 跨平台兼容性好
- 易于调试和测试

### Q2: 如何处理长时间运行的操作？

**A:** 使用流式更新机制：
```typescript
// 发送进度更新
sendNotification('session/update', {
  sessionId,
  update: {
    sessionUpdate: 'agent_thought_chunk',
    content: { type: 'text', text: '正在处理... 50%' },
  },
});
```

### Q3: 如何集成 MCP？

**A:** 在创建会话时配置 MCP 服务器：
```typescript
await sendRequest('session/new', {
  cwd: '/workspace',
  mcpServers: [
    {
      name: 'database',
      command: 'node',
      args: ['./mcp-servers/database-server.js'],
    },
  ],
});
```

### Q4: 如何处理权限缓存？

**A:** 实现简单的缓存机制：
```typescript
const permissionCache = new Map<string, boolean>();

async function checkPermission(toolCall: ToolCall): Promise<boolean> {
  // 检查缓存
  if (permissionCache.has(toolCall.kind)) {
    return permissionCache.get(toolCall.kind)!;
  }

  // 请求用户权限
  const response = await requestPermission(toolCall);
  permissionCache.set(toolCall.kind, response);

  return response;
}
```

### Q5: 如何调试 ACP 通信？

**A:** 使用管道重定向：
```bash
# 手动测试 Agent
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"0.1.0","clientInfo":{"name":"test","version":"1.0"}}}' | claude-code

# 记录所有通信
claude-code 2>/dev/null | tee agent_output.log
```

### Q6: ACP 和 MCP 的区别是什么？

**A:** 

| 维度 | ACP | MCP |
|-----|-----|-----|
| 用途 | 编辑器 ↔ Agent | Model ↔ Tools |
| 通信方向 | 双向 | 主要单向 |
| 作者 | Zed Industries | Anthropic |

两者是互补关系，可以同时使用。

### Q7: 如何实现文件操作的 diff 显示？

**A:** 使用统一的 diff 格式：
```typescript
sendNotification('session/update', {
  sessionId,
  update: {
    sessionUpdate: 'tool_call',
    toolCallId: 'tool-001',
    status: 'in_progress',
    title: '修改文件',
    kind: 'edit',
    content: [{
      type: 'diff',
      diff: `--- a/src/index.ts
+++ b/src/index.ts
@@ -10,7 +10,7 @@
-function add(a, b) {
+function add(a: number, b: number): number {
   return a + b;
 }`,
    }],
    locations: [{ path: '/workspace/src/index.ts' }],
  },
});
```

### Q8: 如何处理错误？

**A:** 使用标准 JSON-RPC 错误格式：
```typescript
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32603,
    "message": "Internal error",
    "data": {
      "stack": "Error: ...\n  at ..."
    }
  }
}
```

### Q9: 如何支持多会话？

**A:** 使用 sessionId 管理：
```typescript
const sessions = new Map<string, Session>();

function handleRequest(request: any) {
  const sessionId = request.params?.sessionId;
  const session = sessions.get(sessionId);
  
  if (!session) {
    sendError(request.id, -32602, 'Invalid session ID');
    return;
  }
  
  // 处理请求...
}
```

### Q10: 如何实现超时处理？

**A:** 使用 Promise.race：
```typescript
async function sendRequestWithTimeout(
  method: string,
  params: any,
  timeout: number = 60000
): Promise<any> {
  return Promise.race([
    sendRequest(method, params),
    new Promise((_, reject) => 
      setTimeout(() => reject(new Error('Timeout')), timeout)
    )
  ]);
}
```

---

## 参考资料

### 官方资源

- [ACP Specification](https://github.com/zed-industries/zed/tree/main/crates/agent_client_protocol)
- [ACP Protocol](./acp_protocol.md) - 协议详细说明
- [LSP Specification](https://microsoft.github.io/language-server-protocol/) - 参考
- [MCP Documentation](https://modelcontextprotocol.io/) - 补充

### 相关项目

- **AionUi** - ACP 编辑器实现示例
- **Claude Code** - ACP Agent 实现
- **Zed Editor** - ACP 的发起者

### 社区

- GitHub Issues: 报告问题和讨论
- Discord: 实时交流
- 文档贡献: 欢迎提交 PR

---

## 附录

### A. 完整示例代码

#### 完整的 ACP 客户端

```typescript
import { spawn, ChildProcess } from 'child_process';
import { EventEmitter } from 'events';

interface PendingRequest {
  resolve: (value: any) => void;
  reject: (error: Error) => void;
  timer: NodeJS.Timeout;
}

export class AcpClient extends EventEmitter {
  private agentProcess: ChildProcess | null = null;
  private pendingRequests: Map<number, PendingRequest> = new Map();
  private requestIdCounter = 0;
  private sessionId: string | null = null;

  async connect(command: string, workingDir: string) {
    this.agentProcess = spawn(command, [], {
      cwd: workingDir,
      env: process.env,
      stdio: ['pipe', 'pipe', 'pipe'],
    });

    this.agentProcess.stdout.on('data', this.handleStdout.bind(this));
    this.agentProcess.stderr.on('data', this.handleStderr.bind(this));
    this.agentProcess.on('exit', () => {
      this.emit('disconnect');
    });

    // 初始化
    await this.initialize();
  }

  private async initialize() {
    const response = await this.sendRequest('initialize', {
      protocolVersion: '0.1.0',
      clientInfo: { name: 'MyEditor', version: '1.0.0' },
    });
    this.emit('initialized', response.result);
  }

  async newSession(cwd: string, mcpServers: any[] = []) {
    const response = await this.sendRequest('session/new', {
      cwd,
      mcpServers,
    });
    this.sessionId = response.result.sessionId;
    this.emit('session_created', this.sessionId);
    return this.sessionId;
  }

  async sendPrompt(prompt: string) {
    return await this.sendRequest('session/prompt', { prompt });
  }

  private async sendRequest(
    method: string,
    params: any,
    timeout: number = 60000
  ): Promise<any> {
    const id = ++this.requestIdCounter;

    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pendingRequests.delete(id);
        reject(new Error(`Request ${method} timeout`));
      }, timeout);

      this.pendingRequests.set(id, { resolve, reject, timer });

      const message = JSON.stringify({
        jsonrpc: "2.0",
        id,
        method,
        params,
      }) + '\n';

      this.agentProcess!.stdin.write(message);
    });
  }

  private handleStdout(data: Buffer) {
    const lines = data
      .toString()
      .split('\n')
      .filter((line) => line.trim());

    for (const line of lines) {
      try {
        const message = JSON.parse(line);

        if ('id' in message && 'result' in message) {
          const pending = this.pendingRequests.get(message.id);
          if (pending) {
            clearTimeout(pending.timer);
            pending.resolve(message);
            this.pendingRequests.delete(message.id);
          }
        } else if ('method' in message) {
          this.handleNotification(message);
        }
      } catch (error) {
        console.error('Failed to parse message:', line);
      }
    }
  }

  private handleNotification(notification: any) {
    const { method, params } = notification;
    this.emit(method, params);
  }

  private handleStderr(data: Buffer) {
    console.error('[Agent stderr]', data.toString());
  }

  async disconnect() {
    if (this.agentProcess) {
      this.agentProcess.kill();
      this.agentProcess = null;
    }
    this.pendingRequests.forEach(req => clearTimeout(req.timer));
    this.pendingRequests.clear();
  }
}

// 使用示例
(async () => {
  const client = new AcpClient();

  client.on('session/update', (params) => {
    console.log('Update:', params.update);
  });

  client.on('end_turn', () => {
    console.log('Turn ended');
  });

  await client.connect('claude-code', '/workspace');
  const sessionId = await client.newSession('/workspace');
  await client.sendPrompt('帮我重构这个函数');
})();
```

### B. 错误代码参考

| 代码 | 名称 | 说明 |
|-----|------|------|
| -32700 | Parse error | JSON 解析错误 |
| -32600 | Invalid Request | 无效的 JSON-RPC 请求 |
| -32601 | Method not found | 方法不存在 |
| -32602 | Invalid params | 无效的参数 |
| -32603 | Internal error | 服务器内部错误 |

### C. 版本历史

| 版本 | 日期 | 变更 |
|-----|------|------|
| 0.1.0 | 2025-01-01 | 初始版本 |

---

**文档结束**

如有问题或建议，欢迎提交 Issue 或 Pull Request。
