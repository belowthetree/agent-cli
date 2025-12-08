use log::info;
use ratatui::{
    widgets::{Block, Borders, Widget},
    style::Stylize,
    text::{Line, Span},
};

/// 命令提示组件
/// 
/// 负责显示和管理命令提示列表，支持翻页和选择
#[derive(Clone)]
pub struct CommandSuggestions {
    /// 所有可用的命令列表
    pub commands: Vec<String>,
    /// 当前选中的命令索引
    pub selected_index: Option<usize>,
    /// 当前显示的命令起始索引（用于翻页）
    pub display_start: usize,
    /// 最大显示数量
    pub max_display: usize,
    /// 是否显示命令提示
    pub visible: bool,
}

impl Default for CommandSuggestions {
    fn default() -> Self {
        Self {
            commands: Vec::new(),
            selected_index: None,
            display_start: 0,
            max_display: 5, // 默认显示5个命令
            visible: false,
        }
    }
}

impl CommandSuggestions {
    /// 创建新的命令提示组件
    pub fn new() -> Self {
        Self::default()
    }

    /// 更新命令列表
    pub fn update_commands(&mut self, commands: &[String], prefix: &str) {
        info!("{:?} {:?}", commands, prefix);
        self.commands = commands
            .iter()
            .filter(|cmd| cmd.starts_with(prefix))
            .cloned()
            .collect();
        self.selected_index = if !self.commands.is_empty() {
            Some(0)
        } else {
            None
        };
        self.display_start = 0;
        self.visible = !self.commands.is_empty();
    }

    /// 隐藏命令提示
    pub fn hide(&mut self) {
        self.visible = false;
        self.selected_index = None;
        self.commands.clear();
        self.display_start = 0;
    }

    /// 选择下一个命令
    pub fn next(&mut self) {
        if let Some(selected) = self.selected_index {
            if selected < self.commands.len() - 1 {
                self.selected_index = Some(selected + 1);
                
                // 如果选中的命令超出了当前显示范围，调整显示起始位置
                if selected + 1 >= self.display_start + self.max_display {
                    self.display_start = (selected + 1).saturating_sub(self.max_display - 1);
                }
            }
        }
    }

    /// 选择上一个命令
    pub fn previous(&mut self) {
        if let Some(selected) = self.selected_index {
            if selected > 0 {
                self.selected_index = Some(selected - 1);
                
                // 如果选中的命令超出了当前显示范围，调整显示起始位置
                if selected - 1 < self.display_start {
                    self.display_start = selected - 1;
                }
            }
        }
    }

    /// 获取当前选中的命令
    pub fn get_selected_command(&self) -> Option<&String> {
        self.selected_index
            .and_then(|idx| self.commands.get(idx))
    }

    /// 计算当前显示的命令范围
    fn display_range(&self) -> (usize, usize) {
        let start = self.display_start;
        let end = std::cmp::min(start + self.max_display, self.commands.len());
        (start, end)
    }

    /// 获取当前显示的命令数量
    pub fn display_count(&self) -> usize {
        let (start, end) = self.display_range();
        end.saturating_sub(start)
    }
}

impl Widget for &CommandSuggestions {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized {
        if !self.visible || self.commands.is_empty() {
            return;
        }

        // 计算显示范围
        let (start, end) = self.display_range();
        let display_count = self.display_count();
        
        if display_count == 0 {
            return;
        }

        // 创建命令提示文本行
        let mut lines: Vec<Line> = Vec::new();
        for i in start..end {
            let cmd = &self.commands[i];
            let is_selected = self.selected_index == Some(i);
            
            // 添加序号前缀
            let prefix = format!("{:2}. ", i + 1);
            
            if is_selected {
                // 选中的命令：黑底白字样式
                let span = Span::styled(
                    format!("{}> {}", prefix, cmd),
                    ratatui::style::Style::new()
                        .fg(ratatui::style::Color::White)
                        .bg(ratatui::style::Color::Black)
                );
                lines.push(Line::from(span));
            } else {
                // 未选中的命令：黄色文本
                let span = Span::styled(
                    format!("{}  {}", prefix, cmd),
                    ratatui::style::Style::new().yellow()
                );
                lines.push(Line::from(span));
            }
        }

        // 创建标题，显示当前页和总页数
        let total_pages = (self.commands.len() + self.max_display - 1) / self.max_display;
        let current_page = if total_pages > 0 {
            self.display_start / self.max_display + 1
        } else {
            1
        };
        
        let title = if total_pages > 1 {
            format!("命令提示 ({}/{}) 第{}/{}页", 
                   display_count, self.commands.len(), 
                   current_page, total_pages)
        } else {
            format!("命令提示 ({}/{})", display_count, self.commands.len())
        };

        // 创建显示框
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .style(ratatui::style::Style::new().light_blue());
        
        let paragraph = ratatui::widgets::Paragraph::new(lines)
            .block(block);
        
        paragraph.render(area, buf);
    }
}
