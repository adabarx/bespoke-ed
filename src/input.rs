use std::{sync::{atomic::AtomicBool, Arc}, time::{Duration, Instant}};

use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, ModifierKeyCode};

use crate::{primatives::Char, AppState, Model, Quit, ARW};

pub fn input_listener(
    model: &'static Model,
    input_tx: Sender<Msg>,
    tick_rate: Duration,
) -> Result<()> {
    let mut last_tick = Instant::now();
    loop {
        if *model.state.read().unwrap() == AppState::ShutDown { break }

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if crossterm::event::poll(timeout)? {
            if let Some(msg) = handle_events(model, event::read()?) {
                input_tx.send(msg)?;
            }
            last_tick = Instant::now();
        }
    };
    Ok(())
}

#[derive(PartialEq, Eq)]
pub enum NormalCommands {
    InsertMode,
    InsertModeAfterCursor,
    NextChar,
    PrevChar,
    NextLine,
    PrevLine,
}

#[derive(PartialEq, Eq)]
pub enum InsertCommand {
    Insert(Char),
    Replace(Char),
    Delete,
}

#[derive(PartialEq, Eq)]
pub enum Msg {
    Normal(NormalCommands),
    Insert(InsertCommand),
    NormalMode,
    ToFirstChild,
    ToParent,
    ToLeftSibling,
    ToRightSibling,
    Reset,
    ShutDown
}

fn handle_events(model: &'static Model, input: Event) -> Option<Msg> {
    match input {
        Event::Key(key) => match key.code {
            KeyCode::Esc => Some(Msg::NormalMode),
            _ => handle_keys(model, key),
        },
        _ => None,
    }
}

pub fn handle_keys(model: &'static Model, key: KeyEvent) -> Option<Msg> {
    match key.kind {
        KeyEventKind::Press => match key.code {
            KeyCode::Modifier(mod_key) =>
                model.mod_keys
                    .write()
                    .unwrap()
                    .push(mod_key),
            _ => (),
        }
        KeyEventKind::Release => match key.code {
            KeyCode::Modifier(mod_key) =>
                model.mod_keys
                    .write()
                    .unwrap()
                    .retain(|k| *k == mod_key),
            _ => (),
        }
        _ => ()
    }
    match model.state.read().unwrap().clone() {
        AppState::Normal => handle_normal(key),
        AppState::Travel => handle_travel(key),
        AppState::Insert => handle_insert(key),
        AppState::ShutDown => Some(Msg::ShutDown),
    }
}

fn handle_normal(key: KeyEvent) -> Option<Msg> {
    match key.kind {
        KeyEventKind::Press => match key.code {
            KeyCode::Char('h') => Some(Msg::Normal(NormalCommands::PrevChar)),
            KeyCode::Char('j') => Some(Msg::Normal(NormalCommands::PrevLine)),
            KeyCode::Char('k') => Some(Msg::Normal(NormalCommands::NextLine)),
            KeyCode::Char('l') => Some(Msg::Normal(NormalCommands::NextChar)),
            _ => None,
        },
        KeyEventKind::Repeat => None,
        KeyEventKind::Release => None,
    }
}

fn handle_travel(key: KeyEvent) -> Option<Msg> {
    match key.kind {
       _ => None,
    }
}

fn handle_insert(key: KeyEvent) -> Option<Msg> {
    match key.kind {
       _ => None,
    }
}

