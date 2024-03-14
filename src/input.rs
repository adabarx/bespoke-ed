
use std::sync::RwLock;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, ModifierKeyCode};

use crate::{primatives::Char, State};

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
    ShutDown
}

pub fn handle_events(
    mod_keys: &'static RwLock<Vec<ModifierKeyCode>>,
    app_state: &'static RwLock<State>,
    input: Event
) -> Option<Msg> {
    match input {
        Event::Key(key) => match key.code {
            KeyCode::Esc => Some(Msg::NormalMode),
            _ => handle_keys(mod_keys, app_state, key),
        },
        _ => None,
    }
}

pub fn handle_keys(
    mod_keys: &'static RwLock<Vec<ModifierKeyCode>>,
    app_state: &'static RwLock<State>,
    key: KeyEvent
) -> Option<Msg> {
    match key.kind {
        KeyEventKind::Press => match key.code {
            KeyCode::Modifier(mod_key) =>
                mod_keys
                    .write().unwrap()
                    .push(mod_key),
            _ => (),
        }
        KeyEventKind::Release => match key.code {
            KeyCode::Modifier(mod_key) =>
                mod_keys
                    .write().unwrap()
                    .retain(|k| *k == mod_key),
            _ => (),
        }
        _ => ()
    }
    match app_state.read().unwrap().clone() {
        State::Normal => Some(Msg::Normal(handle_normal(mod_keys, key)?)),
        State::Insert => Some(Msg::Insert(handle_insert(mod_keys, key)?)),
        State::ShutDown => Some(Msg::ShutDown),
    }
}

fn handle_normal(_mod_keys: &'static RwLock<Vec<ModifierKeyCode>>, key: KeyEvent) -> Option<NormalCommand> {
    match key.kind {
        KeyEventKind::Press => match key.code {
            KeyCode::Char('h') => Some(NormalCommand::PrevChar),
            KeyCode::Char('j') => Some(NormalCommand::PrevLine),
            KeyCode::Char('k') => Some(NormalCommand::NextLine),
            KeyCode::Char('l') => Some(NormalCommand::NextChar),
            KeyCode::Char('q') => Some(NormalCommand::Quit),
            _ => None,
        },
        KeyEventKind::Repeat => None,
        KeyEventKind::Release => None,
    }
}

fn handle_insert(mod_keys: &'static RwLock<Vec<ModifierKeyCode>>, key: KeyEvent) -> Option<InsertCommand> {
    let ctrl = mod_keys.read().unwrap()
        .iter()
        .find(|&k| if let ModifierKeyCode::LeftControl = *k { true } else { false })
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

