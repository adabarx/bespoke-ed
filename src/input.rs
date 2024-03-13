use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, ModifierKeyCode};
use tokio::sync::mpsc::UnboundedSender;

use crate::{primatives::Char, State, Model};

pub async fn input_listener(
    model: &'static Model,
    input_tx: UnboundedSender<Msg>,
    tick_rate: Duration,
) -> Result<()> {
    let mut last_tick = Instant::now();
    loop {
        if *model.state.read().await == State::ShutDown { break }

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if crossterm::event::poll(timeout)? {
            if let Some(msg) = handle_events(model, event::read()?).await {
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

async fn handle_events(model: &'static Model, input: Event) -> Option<Msg> {
    match input {
        Event::Key(key) => match key.code {
            KeyCode::Esc => Some(Msg::NormalMode),
            _ => handle_keys(model, key).await,
        },
        _ => None,
    }
}

pub async fn handle_keys(model: &'static Model, key: KeyEvent) -> Option<Msg> {
    match key.kind {
        KeyEventKind::Press => match key.code {
            KeyCode::Modifier(mod_key) =>
                model.mod_keys
                    .write()
                    .await
                    .push(mod_key),
            _ => (),
        }
        KeyEventKind::Release => match key.code {
            KeyCode::Modifier(mod_key) =>
                model.mod_keys
                    .write()
                    .await
                    .retain(|k| *k == mod_key),
            _ => (),
        }
        _ => ()
    }
    match model.state.read().await.clone() {
        State::Normal => Some(Msg::Normal(handle_normal(model, key).await?)),
        State::Insert => Some(Msg::Insert(handle_insert(model, key).await?)),
        State::ShutDown => Some(Msg::ShutDown),
    }
}

async fn handle_normal(_model: &'static Model, key: KeyEvent) -> Option<NormalCommand> {
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

async fn handle_insert(model: &'static Model, key: KeyEvent) -> Option<InsertCommand> {
    let ctrl = model.mod_keys.read().await
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

