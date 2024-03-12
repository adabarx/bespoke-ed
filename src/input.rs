use std::{ops::ControlFlow, sync::{atomic::AtomicBool, Arc}, time::{Duration, Instant}};

use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, ModifierKeyCode};
use ratatui::style::Style;

use crate::{primatives::Char, State, Model, Quit, ARW};

pub fn input_listener(
    model: &'static Model,
    input_tx: Sender<Msg>,
    tick_rate: Duration,
) -> Result<()> {
    let mut last_tick = Instant::now();
    loop {
        if *model.state.read().unwrap() == State::ShutDown { break }

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
pub enum NormalCommand {
    Quit,
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
    Backspace,
    NewLine,
    NewLineBefore,
    Normal,
}

#[derive(PartialEq, Eq)]
pub enum Msg {
    Normal(NormalCommand),
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
        State::Normal => Some(Msg::Normal(handle_normal(model, key)?)),
        State::Insert => Some(Msg::Insert(handle_insert(model, key)?)),
        State::ShutDown => Some(Msg::ShutDown),
    }
}

fn handle_normal(_model: &'static Model, key: KeyEvent) -> Option<NormalCommand> {
    match key.kind {
        KeyEventKind::Press => match key.code {
            KeyCode::Char('h') => Some(NormalCommand::PrevChar),
            KeyCode::Char('j') => Some(NormalCommand::PrevLine),
            KeyCode::Char('k') => Some(NormalCommand::NextLine),
            KeyCode::Char('l') => Some(NormalCommand::NextChar),
            _ => None,
        },
        KeyEventKind::Repeat => None,
        KeyEventKind::Release => None,
    }
}

fn handle_insert(model: &'static Model, key: KeyEvent) -> Option<InsertCommand> {
    let ctrl = model.mod_keys.read().unwrap()
        .iter()
        .find(|&k| *k == ModifierKeyCode::LeftControl)
        .is_some();

    match key.kind {
        KeyEventKind::Press => match key.code  {
            KeyCode::Char(char) =>
                Some(InsertCommand::Insert(Char { char, ..Default::default() })),
            KeyCode::Backspace => Some(InsertCommand::Backspace),
            KeyCode::Delete => Some(InsertCommand::Delete),
            KeyCode::Enter if ctrl  => Some(InsertCommand::NewLineBefore),
            KeyCode::Enter if !ctrl => Some(InsertCommand::NewLine),
            _ => None,
        }
       _ => None,
    }
}

