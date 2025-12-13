# 🔧 Agent CLI - MCP Protocol Command Line AI Tool

* A lightweight Rust-based MCP client
* A command-line AI tool based on the MCP (Model Context Protocol) protocol, providing streaming chat interaction and tool calling functionality.
* Supports NapCat connection to QQ

[中文版本](../README.md).

## Examples
![](agentcli.gif)

![](tui.png)

## Features

- ✨ Real-time streaming chat responses
- ✨ Supports MCP tool calling and reasoning process display
- ✨ Configurable MCP server connections
- ✨ Built with Rust, high performance and reliable
- ✨ Supports command-line interactive interface
- ✨ Acts as a server to connect with NapCat and respond to QQ @ messages

## 🔧 Internal Tools

The Agent CLI includes the following built-in internal tools that can be used directly in chats:

| Tool Name | Description | Main Functions |
|-----------|-------------|----------------|
| `filesystem` | File system operations tool | Read, write, list files and directories. By default, it can only operate on files within the current working directory. |
| `get_best_tool` | Get best tool recommendation | Analyze user requirements and recommend the most suitable available tools. |
| `choose_tool` | Tool selector | Inform the system and users about the most appropriate tool to use (typically called internally by `get_best_tool`). |

## 📦 Installation Guide

### Install from Source

1. Ensure Rust is installed
2. Clone the repository:
   ```bash
   git clone https://github.com/your-repo/agent-cli.git
   ```
3. Build the project (NapCat is not compiled by default, need to add parameter --features napcat):
   ```bash
   cd agent-cli
   cargo build --release --features napcat
   ```
4. The binary file is located at `target/release/agent-cli`

5. Rename config_temp.json to config.json, fill in your api_key registered on the deepseek official website, ensure you have both config.json and log4rs.yaml files locally
   If you need to use napcat, run with the parameter `--napcat`, then rename napcat_temp.toml to napcat.toml, it will only process messages sent by the target_qq configured

## 💬 Usage Instructions

Basic chat interaction:
```bash
agent-cli -p "Your question or instruction"
```

## ⚙️ Configuration Method

Configuration file is located at `config.json`, specific configuration reference `config_temp.json` file

## Parameter Description

* --prompt User input, if not provided, enters command-line interactive UI mode
* --stream Whether to use streaming, defaults to true
* --use_tool Whether to use tools, defaults to true
* --wait Wait mode, defaults to false. When true, the program processes standard input in a loop, with no context preservation between conversations
* --remote Start remote WebSocket server, specify listening address (e.g., `127.0.0.1:8080`)

## 🌐 Remote Module - External Integration Guide

The Agent CLI provides a Remote module that allows external applications to interact with the AI model through WebSocket protocol. This module supports multiple input types and configuration options, making it easy to integrate into other systems.

### Quick Start

1. **Start Remote Server**:
   ```bash
   agent-cli --remote 127.0.0.1:8080
   ```

2. **Client Connection Example** (Python):
   ```python
   import asyncio
   import websockets
   import json

   async def send_request(request_data):
       async with websockets.connect('ws://127.0.0.1:8080') as websocket:
           request_json = json.dumps(request_data)
           await websocket.send(request_json)
           response_data = await websocket.recv()
           return json.loads(response_data)

   # Send request
   response = asyncio.run(send_request({
       "request_id": "test_001",
       "input": {"Text": "Hello"},
       "stream": False,
       "use_tools": True
   }))
   print(response)
   ```

### Detailed Protocol Documentation

Complete communication protocol documentation: [remote_protocol.md](remote_protocol.md)

Documentation includes:
- Complete protocol specifications
- All message format definitions
- Multiple input type support (text, images, files, instructions, etc.)
- Configuration options explanation
- Usage examples
- Client implementation guides (Python, JavaScript, etc.)
- Error handling and performance recommendations

### Main Features

- **Multiple Input Types**: Supports text, images (base64), files, structured instructions
- **Streaming Responses**: Supports real-time streaming output
- **Tool Calling**: Configurable MCP tool usage
- **Configuration Overrides**: Supports request-level custom configuration
- **Token Statistics**: Returns detailed token usage information

### Integration Scenarios

- **Web Application Backend**: As an AI service provider
- **Desktop Applications**: Integrate AI functionality
- **Automation Scripts**: Batch processing tasks
- **Monitoring Systems**: Intelligent alert analysis
- **Educational Tools**: Intelligent tutoring systems

## 👨‍💻 Development Guide

### Build

```bash
cargo build
```

### Run Tests

```bash
cargo test
```
or double click file “运行Target.bat”

### Log Settings
Set log level and output in `log4rs.yaml`

## 📜 License

[GPL_V3](LICENSE)
