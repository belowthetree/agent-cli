use std::char;

use log::info;
use ratatui::widgets::{Block, Borders, Paragraph, Widget, Wrap};

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
    pub fn add(&mut self, c: char) {
        self.content.push(c);
    }

    pub fn backspace(&mut self) {
        if self.content.len() <= 0 {
            return;
        }
        info!("退格");
        let mut chars: Vec<char> = self.content.chars().collect();
        let _ = chars.pop();
        self.content = chars.into_iter().collect();
    }

    pub fn height(&self)->u16 {
        self.max_height
    }

    pub fn clear(&mut self) {
        self.content.clear();
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