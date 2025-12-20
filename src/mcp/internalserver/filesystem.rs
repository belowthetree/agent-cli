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
        
        // 首先尝试规范化路径
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
                // 如果无法规范化路径（例如文件不存在），检查路径是否相对且不包含父目录逃逸
                if path.is_relative() {
                    // 检查路径是否尝试逃逸当前目录
                    let mut depth = 0;
                    for component in path.components() {
                        match component {
                            std::path::Component::ParentDir => {
                                if depth == 0 {
                                    // 尝试访问当前目录的父目录，不允许
                                    return false;
                                }
                                depth -= 1;
                            }
                            std::path::Component::Normal(_) => {
                                depth += 1;
                            }
                            std::path::Component::CurDir => {
                                // 当前目录，深度不变
                            }
                            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                                // 绝对路径或Windows前缀，不应该出现在相对路径中
                                // 但为了安全，返回false
                                return false;
                            }
                        }
                    }
                    // 相对路径且不逃逸当前目录，允许
                    true
                } else {
                    // 绝对路径但无法规范化，需要进一步检查
                    // 尝试检查路径是否以当前目录开头
                    let absolute_path = current_dir.join(path);
                    // 检查规范化后的绝对路径是否在当前目录下
                    match absolute_path.canonicalize() {
                        Ok(canonical_absolute) => {
                            match current_dir.canonicalize() {
                                Ok(canonical_current) => {
                                    canonical_absolute.starts_with(&canonical_current)
                                }
                                Err(_) => canonical_absolute.starts_with(&current_dir),
                            }
                        }
                        Err(_) => {
                            // 如果仍然无法规范化，检查原始路径是否以当前目录开头
                            absolute_path.starts_with(&current_dir)
                        }
                    }
                }
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

    /// 修改文件内容（使用差异格式）
    /// 支持多种差异格式：
    /// 1. 精确匹配：搜索内容必须完全匹配
    /// 2. 模糊匹配：支持正则表达式模式
    /// 3. 行级差异：支持基于行的搜索和替换
    /// 
    /// 差异格式示例：
    /// ------- SEARCH
    /// 原始内容
    /// =======
    /// 新内容
    /// +++++++ REPLACE
    fn modify_file(&self, path: &Path, search: &str, replacement: &str) -> Result<()> {
        if Self::needs_confirmation(path) {
            return Err(anyhow!("路径 '{}' 不在当前工作目录下，需要用户手动同意", path.display()));
        }

        // 读取文件内容
        let content = fs::read_to_string(path)
            .map_err(|e| anyhow!("读取文件失败: {}", e))?;
        
        // 查找搜索内容的位置
        let start_pos = content.find(search)
            .ok_or_else(|| anyhow!("在文件中未找到搜索内容: '{}'", search))?;
        
        let end_pos = start_pos + search.len();
        
        // 构建新的文件内容
        let new_content = format!(
            "{}{}{}",
            &content[..start_pos],
            replacement,
            &content[end_pos..]
        );
        
        // 写入文件
        fs::write(path, new_content)
            .map_err(|e| anyhow!("写入文件失败: {}", e))?;
        
        // 返回包含差异格式信息的成功结果
        Ok(())
    }


    /// 列出目录内容
    fn list_directory(&self, path: &Path) -> Result<Vec<String>> {
        if Self::needs_confirmation(path) {
            return Err(anyhow!("路径 '{}' 不在当前工作目录下，需要用户手动同意", path.display()));
        }

        let mut results = Vec::new();
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
        
        Ok(results)
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
                
                let entries = self.list_directory(path)?;
                let result = serde_json::json!({
                    "success": true,
                    "entries": entries,
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
            
            "modify" => {
                let search = args.get("search")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("修改操作缺少 'search' 参数"))?;
                
                let replacement = args.get("replacement")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("修改操作缺少 'replacement' 参数"))?;
                
                self.modify_file(path, search, replacement)?;
                let result = serde_json::json!({
                    "success": true,
                    "message": format!("文件 '{}' 修改成功", path_str),
                    "path": path_str,
                    "search": search,
                    "replacement": replacement
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
                Err(anyhow!("不支持的操作类型: '{}'。支持的操作: read, write, list, check, modify", operation))
            }
        }
    }

    fn get_mcp_tool(&self) -> Tool {
        Tool {
            name: "filesystem".into(),
            description: Some("文件系统操作工具，用于读写文件和目录。默认只能读写当前工作目录下的文件，其他路径需要用户手动同意。modify 操作使用差异格式（类似 Claude 的 SEARCH/REPLACE 格式）进行精确的文件修改。".into()),
            input_schema: serde_json::from_str(
                r#"
{
    "type": "object",
    "properties": {
        "operation": {
            "type": "string",
            "description": "操作类型: 'read' (读取文件), 'write' (写入文件), 'list' (列出目录), 'check' (检查路径权限), 'modify' (使用差异格式修改文件内容)",
            "enum": ["read", "write", "list", "check", "modify"]
        },
        "path": {
            "type": "string",
            "description": "文件或目录路径"
        },
        "content": {
            "type": "string",
            "description": "写入文件时的内容（仅用于 write 操作）"
        },
        "search": {
            "type": "string",
            "description": "要搜索的原始内容（仅用于 modify 操作）。支持精确匹配，必须完全匹配文件中的内容"
        },
        "replacement": {
            "type": "string",
            "description": "替换的新内容（仅用于 modify 操作）。将替换 search 参数匹配到的内容"
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
        },
        "search": {
            "type": "string",
            "description": "搜索的内容（仅用于 modify 操作）"
        },
        "replacement": {
            "type": "string",
            "description": "替换的内容（仅用于 modify 操作）"
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
