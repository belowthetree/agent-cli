# 🔧 Agent CLI - MCP协议命令行AI工具

一个基于MCP(Model Context Protocol)协议的命令行AI工具，提供流式聊天交互和工具调用功能。

[English Version](#english-version)

## 功能特性

- ✨ 实时流式聊天响应
- ✨ 支持MCP工具调用和推理过程显示
- ✨ 可配置的MCP服务器连接
- ✨ 基于Rust构建，高性能且可靠

## 📦 安装指南

### 从源码安装

1. 确保已安装Rust(推荐1.70+版本)
2. 克隆仓库：
   ```bash
   git clone https://github.com/your-repo/agent-cli.git
   ```
3. 编译项目：
   ```bash
   cd agent-cli
   cargo build --release
   ```
4. 二进制文件位于`target/release/agent-cli`

## 💬 使用说明

基本聊天交互：
```bash
agent-cli -p "您的问题或指令"
```

## ⚙️ 配置方法

配置文件位于`agent-cli/config.toml`，可配置：
- 默认MCP服务器
- 连接参数
- 日志偏好设置

## 👨‍💻 开发指南

### 编译

```bash
cargo build
```

### 运行测试

```bash
cargo test
```

### 日志设置

通过环境变量设置日志级别：
```bash
RUST_LOG=debug agent-cli --prompt "您的提示"
```

## 📜 许可证

[GPL_V3](LICENSE)

<a name="english-version"></a>
## English Version

For English documentation, please refer to [README_EN.md](README_EN.md).
