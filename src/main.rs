use crossterm::{
    event::{self, KeyCode, KeyEventKind},
    terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
    ExecutableCommand,
};
use ratatui::{
    prelude::{CrosstermBackend, Stylize, Terminal},
    widgets::Paragraph,
};
use std::io::{stdout, stderr};
use anyhow::Result;

fn main() -> Result<()> {
    stdout().execute(EnterAlternateScreen)?;
    let _ = enable_raw_mode();
    crossterm::execute!(stderr(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stderr()))?;
    terminal.clear()?;

    let mut counter = 0;

    loop {
        let _ = terminal.draw(|frame| {
            let area = frame.size();
            frame.render_widget(
                Paragraph::new(format!("Counter {}", counter))
                    .black()
                    .on_blue(),
                area
            );
        })?;

        if event::poll(std::time::Duration::from_millis(4))? {
            if let event::Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('j') => counter += 1,
                        KeyCode::Char('k') => counter -= 1,
                        KeyCode::Char('q') => break,
                        _ => {}
                    }
                }
            }
        }
    }
    let _ = stdout().execute(LeaveAlternateScreen);
    disable_raw_mode()?;
    Ok(())
}
