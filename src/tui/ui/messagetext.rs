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

    pub fn lines(&'_ self)->Vec<Line<'_>> {
        vec![Line::styled(self.msg.content.clone().into_owned(), Style::new().cyan())]
    }
}


impl From<&MessageText> for Text<'_> {
    fn from(value: &MessageText) -> Self {
        Text::from(Line::styled(value.msg.content.clone().into_owned(), Style::new().cyan()))
    }
}

impl Widget for &MessageText {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized {
        Line::raw(self.msg.content.as_ref())
        .render(area, buf);
    }
}
