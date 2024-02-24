use crossterm::event;
use anyhow::{anyhow, bail, Result};
use ratatui::{widgets::Paragraph, Frame};

#[derive(Debug, Default)]
struct Model {
    counter: isize,
    app_state: AppState
}

#[derive(Debug, Default, PartialEq, Eq)]
enum AppState {
    #[default]
    Running,
    Stop
}

enum Msg {
    Increment,
    Decrement,
    Reset,
    Quit
}

fn update(mut model: Model, msg: Msg) -> Model{
    match msg {
        Msg::Increment => model.counter += 1,
        Msg::Decrement => model.counter -= 1,
        Msg::Reset => model.counter = 0,
        Msg::Quit => model.app_state = AppState::Stop,
    }
    model
}

fn view(model: &Model, frame: &mut Frame) {
    frame.render_widget(
        Paragraph::new(format!("Counter: {}", model.counter)),
        frame.size(),
    )
}

fn handle_events() -> Result<Msg> {
    if event::poll(std::time::Duration::from_millis(4))? {
        if let event::Event::Key(key) = event::read()? {
            if key.kind == event::KeyEventKind::Press {
                return match key.code {
                    event::KeyCode::Char('j') => Ok(Msg::Increment),
                    event::KeyCode::Char('k') => Ok(Msg::Decrement),
                    event::KeyCode::Char('r') => Ok(Msg::Reset),
                    event::KeyCode::Char('q') => Ok(Msg::Quit),
                    _ => bail!("no matching key")
                }
            }
        }
    }
    Err(anyhow!("no event"))
}

fn main() -> Result<()> {
    tui::install_panic_hook();

    let mut terminal = tui::init_app()?;

    let mut model = Model::default();

    while model.app_state == AppState::Running {
        terminal.draw(|f| view(&model, f))?;
        
        let msg = handle_events();

        if let Ok(msg) = msg {
            model = update(model, msg);
        }
    }

    tui::teardown_app()
}

mod tui {
    use crossterm::{
        terminal::{
            disable_raw_mode, enable_raw_mode, EnterAlternateScreen,
            LeaveAlternateScreen,
        },
        ExecutableCommand,
    };
    use ratatui::{
        backend::Backend, prelude::{CrosstermBackend, Terminal},
    };
    use std::{io::stdout, panic};
    use anyhow::{Ok, Result};

    pub fn init_app() -> Result<Terminal<impl Backend>> {
        stdout().execute(EnterAlternateScreen)?;
        enable_raw_mode()?;
        let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
        terminal.clear()?;
        Ok(terminal)
    }

    pub fn teardown_app() -> Result<()> {
        stdout().execute(LeaveAlternateScreen)?;
        disable_raw_mode()?;
        Ok(())
    }

    pub fn install_panic_hook() {
        let original_hook = panic::take_hook();
        panic::set_hook(Box::new(move |panic_info| {
            stdout().execute(LeaveAlternateScreen).unwrap();
            disable_raw_mode().unwrap();
            original_hook(panic_info);
        }));
    }
}
