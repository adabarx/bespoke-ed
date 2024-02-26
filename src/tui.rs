use crossterm::{
    event::{self, Event}, 
    terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen,
        LeaveAlternateScreen,
    }, ExecutableCommand
};
use ratatui::{
    backend::Backend, prelude::{CrosstermBackend, Terminal},
};
use std::{io::stdout, panic, time::{Duration, Instant}};

use crossbeam_channel::{Receiver, Sender};
use anyhow::{Ok, Result};
use crate::Msg;

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

pub fn input_listener(
    input_tx: Sender<Event>,
    quit_rx: Receiver<Msg>,
    tick_rate: Duration,
) -> Result<()> {
    let mut last_tick = Instant::now();
    loop {
        if quit_rx.try_recv().is_ok() { break; }

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if crossterm::event::poll(timeout)? {
            input_tx.send(event::read()?)?;
            last_tick = Instant::now();
        }
    };
    Ok(())
}

pub fn view() {}

