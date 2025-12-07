use std::char;
use log::{debug, info};
use ratatui::{widgets::{Block, Borders, Paragraph, Widget, Wrap}, style::Stylize, text::{Line, Span}};
use crate::tui::get_char_width;

#[derive(Clone)]
pub struct InputArea {
    pub content: String,
    pub max_height: u16,
    /// 命令提示列表
    pub command_suggestions: Vec<String>,
    /// 当前选中的命令提示索引
    pub selected_suggestion: Option<usize>,
    /// 是否显示命令提示
    pub show_suggestions: bool,
}

impl Default for InputArea {
    fn default() -> Self {
        Self {
            content: "".into(),
            max_height: 3,
            command_suggestions: Vec::new(),
            selected_suggestion: None,
            show_suggestions: false,
        }
    }
}

impl InputArea {
    pub fn add(&mut self, c: char, idx: usize) {
        self.content.insert(idx, c);
    }

    // 退格，返回退格的字符宽度
    pub fn backspace(&mut self, width: u16)->u16 {
        if self.content.len() <= 0 {
            return 0;
        }
        debug!("退格");
        let mut chars: Vec<char> = self.content.chars().collect();
        let mut width_count = 0;
        let mut res = 0;
        for idx in 0..chars.len() {
            res = get_char_width(chars[idx]);
            width_count += res;
            if width_count >= width {
                chars.remove(idx);
                break;
            }
        }
        self.content = chars.into_iter().collect();
        res
    }

    pub fn height(&self)->u16 {
        self.max_height
    }

    pub fn clear(&mut self) {
        self.content.clear();
        self.hide_suggestions();
    }

    /// 隐藏命令提示
    pub fn hide_suggestions(&mut self) {
        self.show_suggestions = false;
        self.selected_suggestion = None;
        self.command_suggestions.clear();
    }

    /// 更新命令提示
    pub fn update_suggestions(&mut self, commands: &[String], prefix: &str) {
        self.command_suggestions = commands
            .iter()
            .filter(|cmd| cmd.starts_with(prefix))
            .cloned()
            .collect();
        self.show_suggestions = !self.command_suggestions.is_empty();
        if self.show_suggestions {
            self.selected_suggestion = Some(0);
        } else {
            self.selected_suggestion = None;
        }
    }

    /// 选择下一个命令提示
    pub fn next_suggestion(&mut self) {
        if let Some(selected) = self.selected_suggestion {
            if selected < self.command_suggestions.len() - 1 {
                self.selected_suggestion = Some(selected + 1);
            }
        }
        info!{"sugg {:?}", self.selected_suggestion};
    }

    /// 选择上一个命令提示
    pub fn previous_suggestion(&mut self) {
        if let Some(selected) = self.selected_suggestion {
            if selected > 0 {
                self.selected_suggestion = Some(selected - 1);
            }
        }
        info!{"sugg {:?}", self.selected_suggestion};
    }

    /// 获取当前选中的命令
    pub fn get_selected_command(&self) -> Option<&String> {
        self.selected_suggestion
            .and_then(|idx| self.command_suggestions.get(idx))
    }

    /// 检查是否应该显示命令提示
    pub fn should_show_suggestions(&self) -> bool {
        self.show_suggestions && !self.command_suggestions.is_empty()
    }

    pub fn get_content_width(&self)->u16 {
        let mut width = 0;
        for c in self.content.chars() {
            width += get_char_width(c);
        }
        width
    }

    // 获取当前宽度位置的下一个字符的宽度
    pub fn get_width(&self, width: u16)->u16 {
        let chars: Vec<char> = self.content.chars().collect();
        let mut width_count = 0;
        for idx in 0..chars.len() {
            let w = get_char_width(chars[idx]);
            if width_count >= width {
                return w;
            }
            width_count += w;
        }
        0
    }

    // 获取当前宽度位置的上一个字符的宽度
    pub fn get_previous_char_width(&self, width: u16)->u16 {
        let chars: Vec<char> = self.content.chars().collect();
        let mut width_count = 0;
        for idx in 0..chars.len() {
            let w = get_char_width(chars[idx]);
            width_count += w;
            if width_count >= width {
                return w;
            }
        }
        0
    }

    pub fn get_index_by_width(&self, width: u16)->usize {
        let chars: Vec<char> = self.content.chars().collect();
        let mut width_count = 0;
        let mut s = String::new();
        for idx in 0..chars.len() {
            let w = get_char_width(chars[idx]);
            if width_count >= width {
                return s.len();
            }
            s.push(chars[idx]);
            width_count += w;
        }
        s.len()
    }
}

impl Widget for &InputArea {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized {
        // 首先渲染命令提示列表（如果显示）
        if self.should_show_suggestions() {
            // 计算命令提示区域（在输入区域上方）
            let suggestions_height = std::cmp::min(self.command_suggestions.len() as u16, 5); // 最多显示5个
            let suggestions_area = ratatui::layout::Rect {
                x: area.x,
                y: area.y.saturating_sub(suggestions_height),
                width: area.width,
                height: suggestions_height,
            };
            
            // 创建命令提示文本行，每行一个命令
            let mut lines: Vec<Line> = Vec::new();
            for (i, cmd) in self.command_suggestions.iter().enumerate() {
                let is_selected = self.selected_suggestion == Some(i);
                if is_selected {
                    // 选中的命令：黑底白字样式
                    let span = Span::styled(
                        format!("> {}", cmd),
                        ratatui::style::Style::new()
                            .fg(ratatui::style::Color::White)
                            .bg(ratatui::style::Color::Black)
                    );
                    lines.push(Line::from(span));
                } else {
                    // 未选中的命令：黄色文本
                    let span = Span::styled(
                        format!("  {}", cmd),
                        ratatui::style::Style::new().yellow()
                    );
                    lines.push(Line::from(span));
                }
            }
            
            // 仿照MessageBlock样式创建显示框
            let block = ratatui::widgets::Block::default()
                .title("命令提示")
                .borders(ratatui::widgets::Borders::ALL)
                .style(ratatui::style::Style::new().light_blue());
            
            let list = ratatui::widgets::Paragraph::new(lines)
                .block(block);
            list.render(suggestions_area, buf);
        }
        
        // 渲染输入区域
        let block = Block::default()
            .title("输入")
            .borders(Borders::ALL)
            .style(ratatui::style::Style::new().light_blue());
        let para = Paragraph::new(self.content.clone())
            .wrap(Wrap { trim: false})
            .block(block)
            .style(ratatui::style::Style::new().green());
        para.render(area, buf);
    }
}
