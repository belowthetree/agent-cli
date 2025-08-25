use log::{debug, info};
use ratatui::{buffer::Buffer, layout::Rect, style::{Style, Stylize}, text::{Line}, widgets::{Block, Borders, ListItem, Padding, Paragraph, Widget, Wrap}};

use crate::{model::param::ModelMessage, tui::get_char_width};

#[derive(Clone)]
pub struct MessageBlock {
    pub message: ModelMessage,
    pub line_count: u16,
}

impl MessageBlock {
    pub fn new(message: ModelMessage, width: u16)->Self {
        let mut s = Self {
            message,
            line_count: 0,
        };
        s.line_count = s.height(width);
        s
    }

    pub fn height(&self, viewwidth: u16)->u16 {
        // 至少 4 行
        let mut height = 3;
        let mut width_count = 0;
        let mut count = 0;
        for char in self.message.content.chars() {
            let width = get_char_width(char);
            debug!("{} {}", char, width);
            if width_count + width > viewwidth {
                info!("换行！！{}", char);
                height += 1;
                width_count = width_count + width - viewwidth;
            }
            else {
                width_count += width;
            }
            if char == '\n' {
                count += 1;
                width_count = 0;
                height += 1;
            }
        }
        info!("{} {}", count, height);
        height
    }

    pub fn get_display_content(&self, start_line: u16, viewwidth: u16)->String {
        let mut content = String::new();
        let chars = self.message.content.chars();
        // 至少 4 行
        let mut height = 3;
        let mut width_count = 0;
        let start_line = start_line + height;
        for char in chars {
            if height >= start_line {
                content.push(char);
            }
            else {
                let width = get_char_width(char);
                if width_count + width > viewwidth {
                    info!("换行！！ {} {}", char, width);
                    height += 1;
                    width_count = width;
                }
                else {
                    width_count += width;
                }
                if char == '\n' {
                    width_count = 0;
                    height += 1;
                }
            }
        }
        info!("{}", content);
        content
    }

    // 从第 start_line 行开始渲染
    pub fn render_block(&self, area: Rect, buf: &mut Buffer, start_line: u16, viewwidth: u16) {
        let block = Block::default()
            .title(self.message.role.as_str())
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
}

impl Widget for &MessageBlock {
    fn render(self, area: Rect, buf: &mut Buffer)
    where Self: Sized {
        let block = Block::default()
            .title(self.message.role.as_str())
            .padding(Padding::ZERO)
            .style(Style::new().light_blue())
            .borders(Borders::ALL);
        let mut para = Paragraph::new(self.message.content.clone())
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
        ListItem::new(Line::styled(std::borrow::Cow::Owned(value.message.content.clone()), Style::new().cyan()))
    }
}