use crate::{tui::{app::App}};

mod app;
mod appevent;
mod appchat;
mod renderer;
mod state_manager;
mod commands;
mod command_suggestions;
mod option_dialog;
mod ui;

#[allow(unused_imports)]
pub use commands::{TuiCommand, CommandRegistry, init_global_registry, global_registry};

pub async fn run() {
    color_eyre::install().unwrap();
    let term = ratatui::init();
    App::new().run(term).await.unwrap();
    ratatui::restore();
}

pub fn get_char_width(c: char)->u16 {
    unicode_width::UnicodeWidthChar::width(c).unwrap_or(1) as u16
}

pub fn get_str_width(s: &str)->u16 {
    let mut width = 0;
    for char in s.chars() {
        width += get_char_width(char);
    }
    width
}

#[cfg(test)]
mod test {
    use super::*;

    #[allow(unused)]
    async fn test_window() {
        log4rs::init_file("log4rs.yaml", Default::default()).unwrap();
        color_eyre::install().unwrap();
        let term = ratatui::init();
        App::new().run(term).await.unwrap();
        ratatui::restore();
    }
}
