use std::io;

use ratatui::crossterm::event::{self, KeyCode};
use ratatui::style::{Color, Style};
use ratatui::widgets::Paragraph;

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    loop {
        terminal.draw(|frame| {
            frame.render_widget(
                Paragraph::new("nixdev").style(Style::new().fg(Color::Rgb(122, 162, 247))),
                frame.area(),
            );
        })?;
        if let event::Event::Key(key) = event::read()? {
            if key.code == KeyCode::Char('q') {
                break;
            }
        }
    }
    ratatui::restore();
    Ok(())
}
