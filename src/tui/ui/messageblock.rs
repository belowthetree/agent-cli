use log::debug;
use ratatui::{buffer::Buffer, layout::Rect, style::{Style, Stylize}, text::{Line}, widgets::{Block, Borders, ListItem, Padding, Paragraph, Widget, Wrap}};
use serde_json::{Value, Map};

use crate::{model::param::ModelMessage, tui::get_char_width};

#[derive(Clone)]
pub struct MessageBlock {
    pub message: ModelMessage,
    pub line_count: u16,
    cached_width: Option<u16>,
    line_breaks: Vec<usize>, // 缓存每行的结束位置
}

impl MessageBlock {
    pub fn new(message: ModelMessage, width: u16)->Self {
        let mut s = Self {
            message,
            line_count: 0,
            cached_width: None,
            line_breaks: Vec::new(),
        };
        s.line_count = s.calculate_height(width);
        s.cached_width = Some(width);
        s
    }

    pub fn height(&self, viewwidth: u16)->u16 {
        // 如果宽度相同且已缓存，直接返回缓存的高度
        if let Some(cached_width) = self.cached_width {
            if cached_width == viewwidth {
                return self.line_count;
            }
        }
        // 创建一个临时的副本来计算高度
        self.calculate_height_temp(viewwidth)
    }
    
    fn calculate_height_temp(&self, viewwidth: u16)->u16 {
        // 至少 4 行（标题和边框）
        let mut height = 3;
        let ct = self.get_content();
        
        // 使用更高效的方法：按行分割，然后计算每行的宽度
        let lines: Vec<&str> = ct.split('\n').collect();
        
        for line in &lines {
            let mut width_count = 0;
            for char in line.chars() {
                let char_width = get_char_width(char);
                if width_count + char_width > viewwidth {
                    height += 1;
                    width_count = char_width;
                } else {
                    width_count += char_width;
                }
            }
            // 每行结束后增加一行
            height += 1;
        }
        
        // 减去最后一行多算的（因为每行结束后都加了1）
        if !lines.is_empty() {
            height -= 1;
        }
        
        height
    }
    
    fn calculate_height(&mut self, viewwidth: u16)->u16 {
        // 至少 4 行（标题和边框）
        let mut height = 3;
        let ct = self.get_content();
        
        // 清空并重新计算行断点
        self.line_breaks.clear();
        
        // 使用更高效的方法：按行分割，然后计算每行的宽度
        let lines: Vec<&str> = ct.split('\n').collect();
        
        let mut total_chars = 0;
        for line in &lines {
            let mut width_count = 0;
            let line_char_count = line.chars().count();
            
            for i in 0..line_char_count {
                let char = line.chars().nth(i).unwrap();
                let char_width = get_char_width(char);
                if width_count + char_width > viewwidth {
                    height += 1;
                    width_count = char_width;
                    self.line_breaks.push(total_chars + i);
                } else {
                    width_count += char_width;
                }
            }
            // 每行结束后增加一行
            height += 1;
            self.line_breaks.push(total_chars + line_char_count); // 记录行结束位置（字符数）
            total_chars += line_char_count + 1; // +1 用于换行符
        }
        
        // 减去最后一行多算的（因为每行结束后都加了1）
        if !lines.is_empty() {
            height -= 1;
        }
        
        height
    }

    pub fn get_display_content(&self, start_line: u16, viewwidth: u16)->String {
        let ct = self.get_content();
        
        // 如果宽度不同或没有缓存行断点，使用简单方法
        if self.cached_width != Some(viewwidth) || self.line_breaks.is_empty() {
            return self.get_display_content_simple(start_line, viewwidth);
        }
        
        // 至少 4 行（标题和边框）
        let header_lines = 3;
        let start_line = (start_line + header_lines) as usize;
        
        // 如果起始行超过总行数，返回空字符串
        if start_line >= self.line_breaks.len() {
            return String::new();
        }
        
        let mut result = String::new();
        
        // 将字符索引转换为字节索引
        let mut byte_start = 0;
        
        // 快速跳转到起始行对应的字符位置
        let target_char_index = if start_line > 0 {
            self.line_breaks[start_line - 1]
        } else {
            0
        };
        
        // 找到目标字符位置的字节索引
        for (i, (byte_idx, _)) in ct.char_indices().enumerate() {
            if i == target_char_index {
                byte_start = byte_idx;
                break;
            }
        }
        
        // 跳过换行符
        if byte_start < ct.len() && ct.as_bytes()[byte_start] == b'\n' {
            byte_start += 1;
        }
        
        // 只处理从start_line开始的行
        for &break_char_pos in &self.line_breaks[start_line..] {
            // 找到行结束位置的字节索引
            let mut byte_end = ct.len();
            for (i, (byte_idx, _)) in ct.char_indices().enumerate() {
                if i == break_char_pos {
                    byte_end = byte_idx;
                    break;
                }
            }
            
            // 添加从 byte_start 到 byte_end 的字符
            if byte_start < byte_end {
                let slice = &ct[byte_start..byte_end];
                if !slice.is_empty() {
                    if !result.is_empty() {
                        result.push('\n');
                    }
                    result.push_str(slice);
                }
            }
            
            // 更新下一个行的起始位置
            byte_start = byte_end;
            
            // 跳过换行符
            if byte_start < ct.len() && ct.as_bytes()[byte_start] == b'\n' {
                byte_start += 1;
            }
        }
        
        debug!("{}", result);
        result
    }
    
    fn get_display_content_simple(&self, start_line: u16, viewwidth: u16)->String {
        let ct = self.get_content();
        let lines: Vec<&str> = ct.split('\n').collect();
        
        // 至少 4 行（标题和边框）
        let header_lines = 3;
        let start_line = start_line + header_lines;
        
        let mut result = String::new();
        let mut current_line = 0;
        
        for line in lines {
            let mut width_count = 0;
            let mut line_started = false;
            
            for char in line.chars() {
                let char_width = get_char_width(char);
                
                // 检查是否需要换行
                if width_count + char_width > viewwidth {
                    current_line += 1;
                    width_count = char_width;
                    if current_line >= start_line {
                        if line_started {
                            result.push('\n');
                        }
                        result.push(char);
                        line_started = true;
                    }
                } else {
                    width_count += char_width;
                    if current_line >= start_line {
                        result.push(char);
                        line_started = true;
                    }
                }
            }
            
            // 行结束
            current_line += 1;
            if current_line >= start_line && !line.is_empty() {
                result.push('\n');
            }
        }
        
        // 移除最后一个换行符（如果有）
        if result.ends_with('\n') {
            result.pop();
        }
        
        result
    }

    // 从第 start_line 行开始渲染
    pub fn render_block(&self, area: Rect, buf: &mut Buffer, start_line: u16, viewwidth: u16) {
        let block = Block::default()
            .title(self.message.role.as_ref())
            .title_bottom(self.get_bottom_content())
            .padding(Padding::ZERO)
            .style(Style::new().light_blue())
            .borders(Borders::ALL);
        let mut para = Paragraph::new(self.get_display_content(start_line, viewwidth))
            .wrap(Wrap { trim: true})
            .block(block);
        if self.message.role == "user" {
            para = para.style(Style::new().green())
        }
        else {
            para = para.style(Style::new().yellow())
        }
        para.render(area, buf);
    }

    pub fn get_content(&self)->String {
        let mut content = self.message.content.clone().into_owned();
        if let Some(tools) = &self.message.tool_calls {
            if !tools.is_empty() {
                content += "\n工具调用：";
                for tool in tools {
                    let tool_name = &tool.function.name;
                    let tool_args = &tool.function.arguments;
                    
                    // 解析工具参数
                    let tool_info = Self::parse_tool_info(tool_name, tool_args);
                    content += &format!("\n  - {}", tool_info);
                }
            }
        }
        
        content
    }
    
    /// 解析工具信息，特别是filesystem工具的参数
    fn parse_tool_info(tool_name: &str, arguments: &str) -> String {
        if tool_name == "filesystem" {
            // 尝试解析JSON参数
            match serde_json::from_str::<Map<String, Value>>(arguments) {
                Ok(params) => {
                    let operation = params.get("operation")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    
                    let path = params.get("path")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    
                    // 根据操作类型显示不同的信息
                    match operation {
                        "read" => format!("使用工具filesystem读取文件 {}", path),
                        "write" => format!("使用工具filesystem写入文件 {}", path),
                        "list" => format!("使用工具filesystem列出目录 {}", path),
                        "check" => format!("使用工具filesystem检查路径权限 {}", path),
                        "modify" => format!("使用工具filesystem修改文件 {}", path),
                        _ => format!("使用工具filesystem执行{}操作 {}", operation, path),
                    }
                }
                Err(_) => {
                    // 如果JSON解析失败，显示原始信息
                    format!("使用工具filesystem (参数解析失败)")
                }
            }
        } else {
            // 其他工具显示简单信息
            format!("使用工具{}", tool_name)
        }
    }

    pub fn get_bottom_content(&self)->String {
        // 添加token使用信息显示
        if let Some(usage) = &self.message.token_usage {
            return format!(
                "\n\nToken使用: 本次回复消耗 {} tokens (prompt: {}, completion: {}), 总计: {} tokens",
                usage.completion_tokens, usage.prompt_tokens, usage.completion_tokens, usage.total_tokens
            );
        }
        return String::new();
    }
}

impl Widget for &MessageBlock {
    fn render(self, area: Rect, buf: &mut Buffer)
    where Self: Sized {
        let block = Block::default()
            .title(self.message.role.as_ref())
            .title_bottom(self.get_bottom_content())
            .padding(Padding::ZERO)
            .style(Style::new().light_blue())
            .borders(Borders::ALL);
        let content = self.get_content();
        let mut para = Paragraph::new(content)
            .wrap(Wrap { trim: true})
            .block(block);
        if self.message.role == "user" {
            para = para.style(Style::new().green())
        }
        else {
            para = para.style(Style::new().yellow())
        }
        para.render(area, buf);
    }
}

impl From<&MessageBlock> for ListItem<'_> {
    fn from(value: &MessageBlock) -> Self {
        ListItem::new(Line::styled(value.message.content.clone().into_owned(), Style::new().cyan()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_message_block_performance() {
        println!("性能测试开始...");
        
        // 测试1: 创建大量消息块
        println!("\n测试1: 创建100个消息块");
        let start = Instant::now();
        let mut blocks = Vec::new();
        for i in 0..100 {
            let message = ModelMessage {
                role: if i % 2 == 0 { "user".to_string().into() } else { "assistant".to_string().into() },
                content: format!("这是第{}条消息，包含一些文本内容用于测试。这是一个较长的消息，用于测试换行和高度计算。", i).into(),
                think: "".into(),
                name: "".into(),
                tool_call_id: "".into(),
                tool_calls: None,
                token_usage: None,
            };
            let block = MessageBlock::new(message, 80);
            blocks.push(block);
        }
        let duration = start.elapsed();
        println!("创建100个消息块耗时: {:?}", duration);
        
        // 测试2: 计算高度（使用缓存）
        println!("\n测试2: 计算所有块的高度（使用缓存）");
        let start = Instant::now();
        let mut total_height = 0;
        for block in &blocks {
            total_height += block.height(80);
        }
        let duration = start.elapsed();
        println!("计算100个块的高度耗时: {:?}", duration);
        println!("总高度: {} 行", total_height);
        
        // 测试3: 再次计算高度（应该使用缓存）
        println!("\n测试3: 再次计算高度（应该使用缓存）");
        let start = Instant::now();
        let mut total_height2 = 0;
        for block in &blocks {
            total_height2 += block.height(80);
        }
        let duration = start.elapsed();
        println!("再次计算100个块的高度耗时: {:?}", duration);
        println!("总高度: {} 行", total_height2);
        
        // 测试4: 不同宽度的高度计算（不使用缓存）
        println!("\n测试4: 不同宽度的高度计算（不使用缓存）");
        let start = Instant::now();
        let mut total_height3 = 0;
        for block in &blocks {
            total_height3 += block.height(60); // 不同宽度
        }
        let duration = start.elapsed();
        println!("不同宽度计算100个块的高度耗时: {:?}", duration);
        println!("总高度: {} 行", total_height3);
        
        // 测试5: 获取显示内容
        println!("\n测试5: 获取显示内容（从第10行开始）");
        let start = Instant::now();
        for (_i, block) in blocks.iter().enumerate().take(10) {
            let _ = block.get_display_content(10, 80);
        }
        let duration = start.elapsed();
        println!("获取10个块的显示内容耗时: {:?}", duration);
        
        println!("\n性能测试完成！");
        
        // 断言性能指标（可选）
        // 创建100个消息块应该小于100ms
        assert!(duration.as_millis() < 1000, "创建消息块太慢");
    }
}