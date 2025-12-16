use ratatui::{
    widgets::{Block, Borders, Widget},
    style::Stylize,
    text::{Line, Span},
};

use crate::tui::get_str_width;

/// 选项对话框组件
/// 
/// 负责显示和管理选项对话框，支持上下键导航和选择
#[derive(Clone)]
pub struct OptionDialog {
    /// 对话框标题
    pub title: String,
    /// 所有可用的选项列表
    pub options: Vec<String>,
    /// 当前选中的选项索引
    pub selected_index: Option<usize>,
    /// 当前显示的选项起始索引（用于翻页）
    pub display_start: usize,
    /// 最大显示数量
    pub max_display: usize,
    /// 是否显示选项对话框
    pub visible: bool,
    /// 对话框宽度（字符数）
    pub width: u16,
    /// 对话框高度（行数）
    pub height: u16,
    /// 对话框位置（相对于终端窗口）
    pub position_x: u16,
    pub position_y: u16,
}

impl Default for OptionDialog {
    fn default() -> Self {
        Self {
            title: "请选择".to_string(),
            options: Vec::new(),
            selected_index: None,
            display_start: 0,
            max_display: 8, // 默认显示8个选项
            visible: false,
            width: 40,
            height: 12,
            position_x: 0,
            position_y: 0,
        }
    }
}

impl OptionDialog {
    /// 创建新的选项对话框组件
    pub fn new() -> Self {
        Self::default()
    }

    /// 显示选项对话框
    pub fn show(&mut self, title: &str, options: Vec<String>) {
        self.title = title.to_string();
        self.options = options;
        self.selected_index = if !self.options.is_empty() {
            Some(0)
        } else {
            None
        };
        self.display_start = 0;
        self.visible = !self.options.is_empty();
        
        // 自动计算对话框大小
        self.calculate_size();
        // 自动居中对话框
        self.center_dialog();
    }

    /// 隐藏选项对话框
    pub fn hide(&mut self) {
        self.visible = false;
        self.selected_index = None;
        self.options.clear();
        self.display_start = 0;
    }

    /// 选择下一个选项
    pub fn next(&mut self) {
        if let Some(selected) = self.selected_index {
            if selected < self.options.len() - 1 {
                self.selected_index = Some(selected + 1);
                
                // 如果选中的选项超出了当前显示范围，调整显示起始位置
                if selected + 1 >= self.display_start + self.max_display {
                    self.display_start = (selected + 1).saturating_sub(self.max_display - 1);
                }
            }
        }
    }

    /// 选择上一个选项
    pub fn previous(&mut self) {
        if let Some(selected) = self.selected_index {
            if selected > 0 {
                self.selected_index = Some(selected - 1);
                
                // 如果选中的选项超出了当前显示范围，调整显示起始位置
                if selected - 1 < self.display_start {
                    self.display_start = selected - 1;
                }
            }
        }
    }

    /// 获取当前选中的选项
    pub fn get_selected_option(&self) -> Option<&String> {
        self.selected_index
            .and_then(|idx| self.options.get(idx))
    }

    /// 获取当前选中的选项索引
    pub fn get_selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// 计算当前显示的选项范围
    fn display_range(&self) -> (usize, usize) {
        let start = self.display_start;
        let end = std::cmp::min(start + self.max_display, self.options.len());
        (start, end)
    }

    /// 获取当前显示的选项数量
    pub fn display_count(&self) -> usize {
        let (start, end) = self.display_range();
        end.saturating_sub(start)
    }

    /// 计算对话框大小
    fn calculate_size(&mut self) {
        // 计算最大选项宽度
        let mut max_width = get_str_width(&self.title) as usize;
        for option in &self.options {
            let width = get_str_width(option) as usize;
            if width > max_width {
                max_width = width;
            }
        }
        
        // 添加边框和序号前缀的宽度
        self.width = (max_width + 8).min(80) as u16; // 最大80字符宽
        self.height = (self.options.len().min(self.max_display) + 4).min(20) as u16; // 最大20行高
    }

    /// 居中对话框
    fn center_dialog(&mut self) {
        // 假设终端宽度为80，高度为24（这是常见的最小终端尺寸）
        // 在实际使用中，应该从App获取实际的终端尺寸
        let terminal_width: u16 = 80;
        let terminal_height: u16 = 24;
        
        self.position_x = (terminal_width.saturating_sub(self.width)) / 2;
        self.position_y = (terminal_height.saturating_sub(self.height)) / 2;
    }

    /// 设置对话框位置
    pub fn set_position(&mut self, x: u16, y: u16) {
        self.position_x = x;
        self.position_y = y;
    }

    /// 设置对话框大小
    pub fn set_size(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
    }
}

impl Widget for &OptionDialog {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized {
        if !self.visible || self.options.is_empty() {
            return;
        }

        // 创建对话框区域
        let dialog_area = ratatui::layout::Rect {
            x: self.position_x,
            y: self.position_y,
            width: self.width,
            height: self.height,
        };

        // 确保对话框在终端区域内
        if dialog_area.x >= area.width || dialog_area.y >= area.height {
            return;
        }

        // 计算显示范围
        let (start, end) = self.display_range();
        let display_count = self.display_count();
        
        if display_count == 0 {
            return;
        }

        // 创建选项文本行
        let mut lines: Vec<Line> = Vec::new();
        
        // 添加标题行
        let title_line = Line::from(Span::styled(
            format!(" {} ", self.title),
            ratatui::style::Style::new().bold().white()
        ));
        lines.push(title_line);
        
        // 添加空行
        lines.push(Line::from(""));

        for i in start..end {
            let option = &self.options[i];
            let is_selected = self.selected_index == Some(i);
            
            // 添加序号前缀
            let prefix = format!("{:2}. ", i + 1);
            
            if is_selected {
                let mut s = String::from(format!("{}> {}", prefix, option));
                let width = get_str_width(&s);
                // 填充空格以使选项行宽度一致
                for _ in width..self.width.saturating_sub(4) {
                    s += " ";
                }
                // 选中的选项：黑底白字样式
                let span = Span::styled(
                    s,
                    ratatui::style::Style::new()
                        .fg(ratatui::style::Color::White)
                        .bg(ratatui::style::Color::Blue)
                );
                lines.push(Line::from(span));
            } else {
                let mut s = format!("{}  {}", prefix, option);
                let width = get_str_width(&s);
                // 填充空格以使选项行宽度一致
                for _ in width..self.width.saturating_sub(4) {
                    s += " ";
                }
                // 未选中的选项：默认样式
                let span = Span::styled(
                    s,
                    ratatui::style::Style::new().white()
                );
                lines.push(Line::from(span));
            }
        }

        // 添加空行
        lines.push(Line::from(""));
        
        // 添加提示行
        let hint_line = Line::from(vec![
            Span::styled("↑/↓", ratatui::style::Style::new().yellow()),
            Span::raw(" 导航 "),
            Span::styled("Enter", ratatui::style::Style::new().yellow()),
            Span::raw(" 确认 "),
            Span::styled("ESC", ratatui::style::Style::new().yellow()),
            Span::raw(" 取消"),
        ]);
        lines.push(hint_line);

        // 创建显示框
        let block = Block::default()
            .borders(Borders::ALL)
            .style(ratatui::style::Style::new().on_black().light_blue());
        
        let paragraph = ratatui::widgets::Paragraph::new(lines)
            .block(block);
        
        paragraph.render(dialog_area, buf);
    }
}
