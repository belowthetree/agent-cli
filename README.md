# 🔧 Agent CLI - MCP协议命令行AI工具

* 一个轻巧的基于 rust 的 mcp client
* 一个基于MCP(Model Context Protocol)协议的命令行AI工具，提供流式聊天交互和工具调用功能。
* 支持 NapCat 连接 QQ

[English Version](docs/README_EN.md).

## 示例
![](docs/agentcli.gif)

![](docs/tui.png)

## 功能特性

- ✨ 实时流式聊天响应
- ✨ 支持MCP工具调用和推理过程显示
- ✨ 可配置的MCP服务器连接
- ✨ 基于Rust构建，高性能且可靠
- ✨ 支持命令行交互式界面
- ✨ 作为服务端与 NapCat 连接响应 QQ @对话

## 🔧 内部工具

Agent CLI 内置了以下内部工具，可直接在聊天中使用：

| 工具名称 | 描述 | 主要功能 |
|---------|------|---------|
| `filesystem` | 文件系统操作工具 | 读取、写入、列出文件和目录，默认只能操作当前工作目录下的文件 |
| `get_best_tool` | 获取最佳工具推荐 | 根据用户需求分析并推荐最合适的可用工具 |
| `choose_tool` | 工具选择器 | 告诉系统和用户应该使用的最合适的工具（通常由 `get_best_tool` 内部调用） |

这些工具在程序启动时自动启用，无需额外配置。

## 📦 安装指南

### 从源码安装

1. 确保已安装Rust
2. 克隆仓库：
   ```bash
   git clone https://github.com/your-repo/agent-cli.git
   ```
3. 编译项目（NapCat 默认不编译，需要加上参数 --features napcat）：
   ```bash
   cd agent-cli
   cargo build --release
   ```
4. 二进制文件位于`target/release/agent-cli`

5. 将 config_temp.json 改名为 config.json，填入你在 deepseek 官网注册的 api_key，确保你的本地有 config.json、log4rs.yaml 两个文件
   如果你需要使用 napcat，运行的时候加上参数 `--napcat`，然后将 napcat_temp.toml 改名为 napcat.toml，它将只处理配置中的 target_qq 发送的信息

## 💬 使用说明

基本聊天交互：
```bash
agent-cli -p "您的问题或指令"
```

## ⚙️ 配置方法

配置文件位于`config.json`，具体配置参考 `config_temp.json` 文件

## 参数说明

* --promp 用户输入，不填则进入命令行交互 UI 模式
* --stream 是否流式，默认为 true
* --use_tool 是否使用工具，默认为 true
* --wait 等待模式，默认为 false。当为 true 时，程序会在循环中处理标准输入，每次对话不保存上下文

## 👨‍💻 开发指南

### 编译

```bash
cargo build
```

### 运行测试

```bash
cargo test
```
或直接双击运行“运行Target.bat”

### 日志设置
在 `log4rs.yaml` 中设置日志等级、输出

## 📜 许可证

[GPL_V3](LICENSE)
