# 🔧 Agent CLI - MCP Protocol Command Line AI Tool

A command line AI tool based on the Model Context Protocol (MCP), providing streaming chat interaction and tool calling capabilities.

[中文版本](#chinese-version)

## Features

- ✨ Real-time streaming chat responses
- ✨ Support for MCP tool calls and reasoning display
- ✨ Configurable MCP server connections
- ✨ Built with Rust for high performance and reliability

## 📦 Installation

### From Source

1. Ensure you have Rust installed (version 1.70+ recommended)
2. Clone the repository:
   ```bash
   git clone https://github.com/your-repo/agent-cli.git
   ```
3. Build the project:
   ```bash
   cd agent-cli
   cargo build --release
   ```

## 💬 Usage

Basic chat interaction:
```bash
agent-cli -p "Your question or instruction"
```

## ⚙️ Configuration

Configuration files are stored in `~/.config/agent-cli/config.toml`. You can specify:
- Default MCP servers
- Connection parameters
- Logging preferences

## 👨‍💻 Development

### Building

```bash
cargo build
```

### Running Tests

```bash
cargo test
```

### Logging

Set log level with environment variable:
```bash
RUST_LOG=debug agent-cli --prompt "your prompt"
```

## 📜 License

MIT

<a name="chinese-version"></a>
## Chinese Version

For Chinese documentation, please refer to [README.md](README.md).
