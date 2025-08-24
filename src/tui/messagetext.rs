use ratatui::{style::{Style, Stylize}, text::{Line, Text}, widgets::Widget};

use crate::model::param::ModelMessage;

pub struct MessageText {
    pub msg: ModelMessage,
}

impl MessageText {
    pub fn new(msg: ModelMessage)->Self {
        Self {
            msg,
        }
    }

    pub fn lines(&self)->Vec<Line> {
        vec![Line::styled(std::borrow::Cow::Owned(self.msg.content.clone()), Style::new().cyan())]
    }
}


impl From<&MessageText> for Text<'_> {
    fn from(value: &MessageText) -> Self {
        Text::from(Line::styled(std::borrow::Cow::Owned(value.msg.content.clone()), Style::new().cyan()))
    }
}

impl Widget for &MessageText {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized {
        Line::raw(self.msg.content.as_str())
        .render(area, buf);
    }
}