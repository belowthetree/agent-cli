use std::time::Duration;
use std::path::Path;
use async_trait::async_trait;
use log::info;
use rmcp::model::{Annotated, CallToolResult, RawContent, RawTextContent, Tool};
use serde_json::{Map, Value};
use anyhow::{anyhow, Result};
use tokio::process::Command as TokioCommand;
use tokio::time::timeout;

use crate::mcp::internalserver::InternalTool;

#[derive(Debug)]
pub struct ShellCommandTool;

impl ShellCommandTool {
    /// 验证命令安全性，防止危险操作
    fn validate_command(command: &str) -> Result<()> {
        let dangerous_patterns = [
            "rm -rf",
            "rm -rf /",
            "chmod -R 777 /",
            "dd if=",
            "> /dev/",
            "< /dev/",
            "mv /",
            "cp /etc/",
            "cat /etc/",
            "echo > /etc/",
            "rm -R",
            "rm -R /",
        ];

        let lower_command = command.to_lowercase();
        for pattern in &dangerous_patterns {
            if lower_command.contains(&pattern.to_lowercase()) {
                return Err(anyhow!("命令包含危险操作: {}", pattern));
            }
        }

        // 检查是否包含过多的管道和重定向，防止复杂命令
        let pipe_count = command.matches('|').count();
        let redirect_count = command.matches('>').count() + command.matches('<').count();
        
        if pipe_count > 5 || redirect_count > 5 {
            return Err(anyhow!("命令过于复杂，包含过多管道或重定向操作"));
        }

        Ok(())
    }

    /// 执行shell命令
    async fn execute_command(&self, cmd_str: &str, working_dir: Option<&str>, timeout_sec: u64) -> Result<String> {
        // 验证命令安全性
        Self::validate_command(cmd_str)?;

        let mut command = if cfg!(windows) {
            let mut cmd = TokioCommand::new("cmd");
            cmd.arg("/C").arg(cmd_str);
            cmd
        } else {
            let mut cmd = TokioCommand::new("sh");
            cmd.arg("-c").arg(cmd_str);
            cmd
        };

        // 设置工作目录
        if let Some(dir) = working_dir {
            if !Path::new(dir).exists() {
                return Err(anyhow!("工作目录不存在: {}", dir));
            }
            command.current_dir(dir);
        }

        // 设置超时并执行命令
        let result = timeout(
            Duration::from_secs(timeout_sec),
            command.output()
        ).await;

        match result {
            Ok(output_result) => {
                match output_result {
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        let exit_code = output.status.code().unwrap_or(-1);

                        let result = serde_json::json!({
                            "success": true,
                            "exit_code": exit_code,
                            "stdout": stdout.to_string(),
                            "stderr": stderr.to_string(),
                            "command": cmd_str,
                            "working_dir": working_dir.unwrap_or(std::env::current_dir().unwrap().to_string_lossy().as_ref())
                        });

                        Ok(result.to_string())
                    }
                    Err(e) => {
                        Err(anyhow!("命令执行失败: {}", e))
                    }
                }
            }
            Err(_) => {
                Err(anyhow!("命令执行超时 ({}秒)", timeout_sec))
            }
        }
    }
}

#[async_trait]
impl InternalTool for ShellCommandTool {
    async fn call(&self, args: Map<String, Value>) -> Result<CallToolResult> {
        info!("ShellCommandTool 调用参数: {:?}", args);
        
        // 解析命令参数
        let command = args.get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("缺少 'command' 参数"))?;
        
        // 解析可选参数
        let working_dir = args.get("working_dir")
            .and_then(|v| v.as_str());
        
        let timeout_sec = args.get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(30); // 默认30秒超时

        // 验证超时时间范围
        if timeout_sec == 0 || timeout_sec > 300 {
            return Err(anyhow!("超时时间必须在1-300秒之间"));
        }

        // 执行命令
        let result = self.execute_command(command, working_dir, timeout_sec).await?;
        
        Ok(CallToolResult {
            content: vec![Annotated::new(
                RawContent::Text(RawTextContent {
                    text: result,
                }),
                None,
            )],
            structured_content: None,
            is_error: None,
        })
    }

    fn get_mcp_tool(&self) -> Tool {
        Tool {
            name: "shell_command".into(),
            description: Some("执行shell命令的工具。支持在指定工作目录执行命令，具有安全验证和超时控制。".into()),
            input_schema: serde_json::from_str(
                r#"
{
    "type": "object",
    "properties": {
        "command": {
            "type": "string",
            "description": "要执行的shell命令"
        },
        "working_dir": {
            "type": "string",
            "description": "命令执行的工作目录，默认为当前目录"
        },
        "timeout": {
            "type": "integer",
            "description": "命令执行超时时间（秒），范围1-300，默认30秒",
            "minimum": 1,
            "maximum": 300
        }
    },
    "required": ["command"]
}
"#).unwrap(),
            output_schema: Some(serde_json::from_str(
                r#"
{
    "type": "object",
    "properties": {
        "success": {
            "type": "boolean",
            "description": "命令是否执行成功"
        },
        "exit_code": {
            "type": "integer",
            "description": "命令退出码"
        },
        "stdout": {
            "type": "string",
            "description": "标准输出"
        },
        "stderr": {
            "type": "string",
            "description": "标准错误输出"
        },
        "command": {
            "type": "string",
            "description": "执行的命令"
        },
        "working_dir": {
            "type": "string",
            "description": "工作目录"
        }
    },
    "required": ["success", "exit_code", "stdout", "stderr", "command", "working_dir"]
}
"#).unwrap()),
            annotations: None,
        }
    }

    fn name(&self) -> String {
        "shell_command".into()
    }
}
