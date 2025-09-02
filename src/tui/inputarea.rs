use std::char;
use log::{debug};
use ratatui::widgets::{Block, Borders, Paragraph, Widget, Wrap};
use crate::tui::get_char_width;

#[derive(Clone)]
pub struct InputArea {
    pub content: String,
    pub max_height: u16,
}

impl Default for InputArea {
    fn default() -> Self {
        Self {
            content: "".into(),
            max_height: 3
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
        let block = Block::default()
            .title("输入")
            .borders(Borders::ALL);
        let para = Paragraph::new(self.content.clone())
            .wrap(Wrap { trim: false})
            .block(block);
        para.render(area, buf);
    }
}