use std::sync::{Arc, Mutex, OnceLock};

use futures::{pin_mut, StreamExt};
use log::info;
use ratatui::{crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers}, layout::{self, Constraint, Direction}, style::{Color, Style, Stylize}, text::Line, widgets::{Paragraph, Wrap}};
use crate::{chat::{Chat, StreamedChatResponse}, client::handle_output, config, model::param::{ModelMessage, ToolCall, ToolCallFunction}, prompt::CHAT_PROMPT, tui::{app::App, messageblock::MessageBlock}};

mod messageblock;
mod app;
mod messagetext;
mod inputarea;

pub async fn run() {
    // element!(Render)
    // .fullscreen()
    // .await
    // .expect("渲染命令行窗口失败");
    let mut chat = Chat::default();
    let term = ratatui::init();
    let app = App::new().run(term);
    ratatui::restore();
}

pub fn get_char_width(c: char)->u16 {
    unicode_width::UnicodeWidthChar::width(c).unwrap_or(1) as u16
}

#[cfg(test)]
mod test {
    use crate::{model::param::ModelMessage, tui::{messageblock::MessageBlock, run}};
    use super::*;
    use ratatui::{
        style::{Color, Style, Stylize},
        text::Line, widgets::{Block, Borders, Paragraph},
    };

    #[tokio::test]
    async fn test_window() {
        log4rs::init_file("log4rs.yaml", Default::default()).unwrap();
        // element!(Render)
        // .fullscreen()
        // .await
        // .expect("渲染命令行窗口失败");
        color_eyre::install();
        let term = ratatui::init();
        let app = App::new().run(term).await;
        ratatui::restore();
    }
}
