use std::char;
use log::{debug, info};
use ratatui::{widgets::{Block, Borders, Paragraph, Widget, Wrap}, style::Stylize};
use crate::tui::{get_char_width, command_suggestions::CommandSuggestions};

#[derive(Clone)]
pub struct InputArea {
    pub content: String,
    pub max_height: u16,
    /// 命令提示组件
    pub command_suggestions: CommandSuggestions,
}

impl Default for InputArea {
    fn default() -> Self {
        Self {
            content: "".into(),
            max_height: 3,
            command_suggestions: CommandSuggestions::new(),
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

    // pub fn height(&self)->u16 {
    //     let display_count = self.command_suggestions.display_count();
    //     if display_count <= 0 {
    //         self.max_height
    //     } else {
    //         let suggestions_height = display_count as u16 + 2; // +2 用于边框
    //         self.max_height + suggestions_height
    //     }
    // }
    pub fn height(&self)->u16 {
        self.max_height
    }

    pub fn clear(&mut self) {
        self.content.clear();
        self.hide_suggestions();
    }

    /// 隐藏命令提示
    pub fn hide_suggestions(&mut self) {
        self.command_suggestions.hide();
    }

    /// 更新命令提示
    pub fn update_suggestions(&mut self, commands: &[String], prefix: &str) {
        self.command_suggestions.update_commands(commands, prefix);
    }

    /// 选择下一个命令提示
    pub fn next_suggestion(&mut self) {
        self.command_suggestions.next();
        info!{"sugg {:?}", self.command_suggestions.selected_index};
    }

    /// 选择上一个命令提示
    pub fn previous_suggestion(&mut self) {
        self.command_suggestions.previous();
        info!{"sugg {:?}", self.command_suggestions.selected_index};
    }

    /// 获取当前选中的命令
    pub fn get_selected_command(&self) -> Option<&String> {
        self.command_suggestions.get_selected_command()
    }

    /// 检查是否应该显示命令提示
    pub fn should_show_suggestions(&self) -> bool {
        self.command_suggestions.visible && !self.command_suggestions.commands.is_empty()
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
        let mut area = area;
        // 首先渲染命令提示列表（如果显示）
        if self.should_show_suggestions() {
            // 计算命令提示区域（在输入区域上方）
            // 命令提示的高度取决于要显示的命令数量
            let display_count = self.command_suggestions.display_count();
            let suggestions_height = display_count as u16 + 2; // +2 用于边框
            
            // 确保有足够的空间显示命令提示
            if suggestions_height > 0 && area.y >= suggestions_height {
                let suggestions_area = ratatui::layout::Rect {
                    x: area.x,
                    y: area.y.saturating_sub(suggestions_height),
                    width: area.width,
                    height: suggestions_height,
                };
                
                // 渲染命令提示组件
                let _ = &self.command_suggestions.render(suggestions_area, buf);
            }
            area.height = area.height.saturating_sub(suggestions_height);
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
