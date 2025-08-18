# Agent-CLI

[![Rust](https://img.shields.io/badge/rust-1.78.0-orange.svg)](https://www.rust-lang.org/)
[![Crates.io](https://img.shields.io/crates/v/agent-cli.svg)](https://crates.io/crates/agent-cli)
[![License](https://img.shields.io/crates/l/agent-cli.svg)](https://opensource.org/licenses/MIT)

`agent-cli` is a command-line interface (CLI) tool that allows you to interact with a large language model (LLM) to perform various tasks. It supports streaming responses, tool calls, and reasoning.

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) >= 1.78.0

### Installation

1. Clone the repository:
   ```bash
   git clone https://github.com/your-username/agent-cli.git
   ```
2. Change into the project directory:
   ```bash
   cd agent-cli
   ```
3. Build the project:
   ```bash
   cargo build --release
   ```

## Usage

To use the `agent-cli`, run the following command:

```bash
cargo run -- --prompt "Your prompt here"
```

### Examples

- Get a simple response:
  ```bash
  cargo run -- --prompt "Hello, world!"
  ```
- Get a response with a value:
  ```bash
  cargo run -- --prompt "What is the capital of" --value "France"
  ```

## Configuration

The `agent-cli` uses a `config.toml` file for configuration. You can create a `config.toml` file in the project root with the following content:

```toml
# Your configuration here
```

## Contributing

Contributions are welcome! Please feel free to submit a pull request or open an issue.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.