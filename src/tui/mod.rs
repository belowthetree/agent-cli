use crate::{tui::{app::App}};

mod messageblock;
mod app;
mod messagetext;
mod inputarea;
mod appevent;
mod appchat;
mod renderer;
mod state_manager;

pub async fn run() {
    color_eyre::install().unwrap();
    let term = ratatui::init();
    App::new().run(term).await.unwrap();
    ratatui::restore();
}

pub fn get_char_width(c: char)->u16 {
    unicode_width::UnicodeWidthChar::width(c).unwrap_or(1) as u16
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
