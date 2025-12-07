use std::path::{Path, PathBuf};
use std::fs;
use async_trait::async_trait;
use log::info;
use rmcp::model::{Annotated, CallToolResult, RawContent, RawTextContent, Tool};
use serde_json::{Map, Value};
use anyhow::{anyhow, Result};

use crate::mcp::internalserver::InternalTool;

#[derive(Debug)]
pub struct FileSystemTool;

impl FileSystemTool {
    /// 获取当前工作目录
    fn get_current_dir() -> PathBuf {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    }

    /// 检查路径是否在当前工作目录下
    fn is_path_allowed(path: &Path) -> bool {
        let current_dir = Self::get_current_dir();
        
        // 特殊处理当前目录 "."
        if path == Path::new(".") || path == Path::new("./") {
            return true;
        }
        
        match path.canonicalize() {
            Ok(canonical_path) => {
                // 规范化当前目录进行比较
                match current_dir.canonicalize() {
                    Ok(canonical_current_dir) => {
                        canonical_path.starts_with(&canonical_current_dir)
                    }
                    Err(_) => {
                        // 如果无法规范化当前目录，使用原始路径比较
                        canonical_path.starts_with(&current_dir)
                    }
                }
            }
            Err(_) => {
                // 如果无法规范化路径，检查相对路径是否在当前目录下
                let relative_path = current_dir.join(path);
                relative_path.exists()
            }
        }
    }

    /// 检查路径是否需要用户确认
    fn needs_confirmation(path: &Path) -> bool {
        !Self::is_path_allowed(path)
    }

    /// 读取文件内容
    fn read_file(&self, path: &Path) -> Result<String> {
        if Self::needs_confirmation(path) {
            return Err(anyhow!("路径 '{}' 不在当前工作目录下，需要用户手动同意", path.display()));
        }
        
        fs::read_to_string(path)
            .map_err(|e| anyhow!("读取文件失败: {}", e))
    }

    /// 写入文件内容
    fn write_file(&self, path: &Path, content: &str) -> Result<()> {
        if Self::needs_confirmation(path) {
            return Err(anyhow!("路径 '{}' 不在当前工作目录下，需要用户手动同意", path.display()));
        }

        // 确保目录存在
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| anyhow!("创建目录失败: {}", e))?;
        }

        fs::write(path, content)
            .map_err(|e| anyhow!("写入文件失败: {}", e))
    }

    /// 列出目录内容
    fn list_directory(&self, path: &Path, recursive: bool) -> Result<Vec<String>> {
        if Self::needs_confirmation(path) {
            return Err(anyhow!("路径 '{}' 不在当前工作目录下，需要用户手动同意", path.display()));
        }

        let mut results = Vec::new();
        
        if recursive {
            Self::list_directory_recursive(path, &mut results, 0)?;
        } else {
            let entries = fs::read_dir(path)
                .map_err(|e| anyhow!("读取目录失败: {}", e))?;
            
            for entry in entries {
                let entry = entry.map_err(|e| anyhow!("读取目录项失败: {}", e))?;
                let path = entry.path();
                let name = path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                
                if path.is_dir() {
                    results.push(format!("{}/", name));
                } else {
                    results.push(name);
                }
            }
        }
        
        Ok(results)
    }

    /// 递归列出目录内容
    fn list_directory_recursive(path: &Path, results: &mut Vec<String>, depth: usize) -> Result<()> {
        let entries = fs::read_dir(path)
            .map_err(|e| anyhow!("读取目录失败: {}", e))?;
        
        for entry in entries {
            let entry = entry.map_err(|e| anyhow!("读取目录项失败: {}", e))?;
            let entry_path = entry.path();
            let relative_path = entry_path.strip_prefix(path)
                .unwrap_or(&entry_path);
            
            let prefix = "  ".repeat(depth);
            let display_path = relative_path.to_string_lossy();
            
            if entry_path.is_dir() {
                results.push(format!("{}{}/", prefix, display_path));
                Self::list_directory_recursive(&entry_path, results, depth + 1)?;
            } else {
                results.push(format!("{}{}", prefix, display_path));
            }
        }
        
        Ok(())
    }
}

#[async_trait]
impl InternalTool for FileSystemTool {
    async fn call(&self, args: Map<String, Value>) -> Result<CallToolResult> {
        info!("FileSystemTool 调用参数: {:?}", args);
        
        // 解析操作类型
        let operation = args.get("operation")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("缺少 'operation' 参数"))?;
        
        let path_str = args.get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("缺少 'path' 参数"))?;
        
        let path = Path::new(path_str);
        
        match operation {
            "read" => {
                let content = self.read_file(path)?;
                let result = serde_json::json!({
                    "success": true,
                    "content": content,
                    "path": path_str
                });
                
                Ok(CallToolResult {
                    content: vec![Annotated::new(
                        RawContent::Text(RawTextContent {
                            text: result.to_string(),
                        }),
                        None,
                    )],
                    structured_content: None,
                    is_error: None,
                })
            }
            
            "write" => {
                let content = args.get("content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("写入操作缺少 'content' 参数"))?;
                
                self.write_file(path, content)?;
                let result = serde_json::json!({
                    "success": true,
                    "message": format!("文件 '{}' 写入成功", path_str),
                    "path": path_str
                });
                
                Ok(CallToolResult {
                    content: vec![Annotated::new(
                        RawContent::Text(RawTextContent {
                            text: result.to_string(),
                        }),
                        None,
                    )],
                    structured_content: None,
                    is_error: None,
                })
            }
            
            "list" => {
                let recursive = args.get("recursive")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                
                let entries = self.list_directory(path, recursive)?;
                let result = serde_json::json!({
                    "success": true,
                    "entries": entries,
                    "path": path_str,
                    "recursive": recursive
                });
                
                Ok(CallToolResult {
                    content: vec![Annotated::new(
                        RawContent::Text(RawTextContent {
                            text: result.to_string(),
                        }),
                        None,
                    )],
                    structured_content: None,
                    is_error: None,
                })
            }
            
            "check" => {
                let is_allowed = Self::is_path_allowed(path);
                let needs_confirmation = Self::needs_confirmation(path);
                let result = serde_json::json!({
                    "success": true,
                    "path": path_str,
                    "is_allowed": is_allowed,
                    "needs_confirmation": needs_confirmation,
                    "current_directory": Self::get_current_dir().to_string_lossy().to_string()
                });
                
                Ok(CallToolResult {
                    content: vec![Annotated::new(
                        RawContent::Text(RawTextContent {
                            text: result.to_string(),
                        }),
                        None,
                    )],
                    structured_content: None,
                    is_error: None,
                })
            }
            
            _ => {
                Err(anyhow!("不支持的操作类型: '{}'。支持的操作: read, write, list, check", operation))
            }
        }
    }

    fn get_mcp_tool(&self) -> Tool {
        Tool {
            name: "filesystem".into(),
            description: Some("文件系统操作工具，用于读写文件和目录。默认只能读写当前工作目录下的文件，其他路径需要用户手动同意。".into()),
            input_schema: serde_json::from_str(
                r#"
{
    "type": "object",
    "properties": {
        "operation": {
            "type": "string",
            "description": "操作类型: 'read' (读取文件), 'write' (写入文件), 'list' (列出目录), 'check' (检查路径权限)",
            "enum": ["read", "write", "list", "check"]
        },
        "path": {
            "type": "string",
            "description": "文件或目录路径"
        },
        "content": {
            "type": "string",
            "description": "写入文件时的内容（仅用于 write 操作）"
        },
        "recursive": {
            "type": "boolean",
            "description": "是否递归列出目录（仅用于 list 操作，默认 false）"
        }
    },
    "required": ["operation", "path"]
}
"#).unwrap(),
            output_schema: Some(serde_json::from_str(
                r#"
{
    "type": "object",
    "properties": {
        "success": {
            "type": "boolean",
            "description": "操作是否成功"
        },
        "message": {
            "type": "string",
            "description": "操作结果消息"
        },
        "content": {
            "type": "string",
            "description": "读取的文件内容（仅用于 read 操作）"
        },
        "entries": {
            "type": "array",
            "items": {
                "type": "string"
            },
            "description": "目录列表（仅用于 list 操作）"
        },
        "path": {
            "type": "string",
            "description": "操作的文件路径"
        },
        "is_allowed": {
            "type": "boolean",
            "description": "路径是否在当前工作目录下（仅用于 check 操作）"
        },
        "needs_confirmation": {
            "type": "boolean",
            "description": "是否需要用户确认（仅用于 check 操作）"
        },
        "current_directory": {
            "type": "string",
            "description": "当前工作目录（仅用于 check 操作）"
        }
    },
    "required": ["success"]
}
"#).unwrap()),
            annotations: None,
        }
    }

    fn name(&self) -> String {
        "filesystem".into()
    }
}
