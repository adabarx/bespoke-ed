use std::{sync::{atomic::AtomicBool, Arc}, time::{Duration, Instant}};

use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};

use crate::Quit;


pub fn input_listener(
    input_tx: Sender<Msg>,
    quit_tx: Sender<Quit>,
    tick_rate: Duration,
) -> Result<()> {
    let mut last_tick = Instant::now();
    loop {
        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if crossterm::event::poll(timeout)? {
            if let Some(msg) = handle_events(event::read()?) {
                if msg == Msg::Quit { 
                    quit_tx.send(Quit)?;
                    break;
                }
                input_tx.send(msg)?;
            }
            last_tick = Instant::now();
        }
    };
    Ok(())
}

#[derive(PartialEq, Eq)]
pub enum Msg {
    ToFirstChild,
    ToParent,
    ToLeftSibling,
    ToRightSibling,
    Reset,
    Quit
}

fn handle_events(input: Event) -> Option<Msg> {
    match input {
        Event::Key(key) => handle_keys(key),
        _ => None,
    }
}

pub fn handle_keys(key: KeyEvent) -> Option<Msg> {
    match key.kind {
        KeyEventKind::Press => match key.code {
            KeyCode::Char('j') => Some(Msg::ToFirstChild),
            KeyCode::Char('k') => Some(Msg::ToParent),
            KeyCode::Char('h') => Some(Msg::ToLeftSibling),
            KeyCode::Char('l') => Some(Msg::ToRightSibling),
            KeyCode::Char('r') => Some(Msg::Reset),
            KeyCode::Char('q') => Some(Msg::Quit),
            _ => None,
        },
        KeyEventKind::Repeat => None,
        KeyEventKind::Release => None,
    }
}

