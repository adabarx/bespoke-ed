
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, ModifierKeyCode};
use ratatui::layout::Rect;
use tokio::{sync::{mpsc::UnboundedSender, RwLock}, task::JoinHandle, time::Instant};

use crate::{primatives::Root, State};

pub fn input_thread_init(
    state: &'static RwLock<State>,
    root: &'static RwLock<Root>,
    input_tx: UnboundedSender<Command>,
    deadline: u64
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let tick_rate = Duration::from_nanos(deadline);
        let mut last_tick = Instant::now();
        let mut mod_keys = Vec::new();
        loop {
            if *state.read().await == State::ShutDown { break }

            let timeout = tick_rate.saturating_sub(last_tick.elapsed());
            if crossterm::event::poll(timeout).unwrap() {
                let event = event::read().unwrap();
                
                match event {
                    Event::FocusLost => (),
                    Event::FocusGained => (),
                    Event::Resize(columns, rows) => root.write().await.area = Rect::new(0, 0, columns, rows),
                    _ => (),
                }

                let msg = match *state.read().await {
                    State::Normal => handle_normal(&mut mod_keys, event).await,
                    State::Insert => handle_insert(&mut mod_keys, event).await,
                    State::Travel => handle_travel(&mut mod_keys, event).await,
                    State::ShutDown => break,
                };
                if let Some(msg) = msg {
                    input_tx.send(msg).unwrap();
                }
                last_tick = Instant::now();
            }
        };
    })
}

#[derive(PartialEq, Eq)]
pub enum Command {
    Insert(char),
    NormalMode,
    InsertMode,
    TravelMode,
    ToFirstChild,
    ToParent,
    ToLeftSibling,
    ToRightSibling,
    Reset,
    ShutDown,
    PrevChar,
    PrevLine,
    NextLine,
    NextChar,
    ToLastChild,
    ToMiddleChild,
}

pub async fn handle_normal(
    _mod_keys: &mut Vec<ModifierKeyCode>,
    input: Event
) -> Option<Command> {
    // let shift = mod_keys.read().await.iter().find(|&k| *k == ModifierKeyCode::LeftShift || *k == ModifierKeyCode::RightShift).is_some();
    // let ctrl = mod_keys.read().await.iter()
    //     .find(|&k| *k == ModifierKeyCode::LeftControl || *k == ModifierKeyCode::RightControl)
    //     .is_some();
    // let alt = mod_keys.read().await.iter().find(|&k| *k == ModifierKeyCode::LeftAlt || *k == ModifierKeyCode::RightAlt).is_some();
    // let meta = mod_keys.read().await.iter().find(|&k| *k == ModifierKeyCode::LeftMeta || *k == ModifierKeyCode::RightMeta).is_some();
    // let super_ = mod_keys.read().await.iter().find(|&k| *k == ModifierKeyCode::LeftSuper || *k == ModifierKeyCode::RightSuper).is_some();
    // let hyper = mod_keys.read().await.iter().find(|&k| *k == ModifierKeyCode::LeftHyper || *k == ModifierKeyCode::RightHyper).is_some();
    match input {
        Event::Key(key) => match key.code {
            KeyCode::Esc => Some(Command::ShutDown),
            KeyCode::Char('i') => Some(Command::InsertMode),
            KeyCode::Char('t') => Some(Command::TravelMode),
            KeyCode::Char('h') => Some(Command::PrevChar),
            KeyCode::Char('j') => Some(Command::PrevLine),
            KeyCode::Char('k') => Some(Command::NextLine),
            KeyCode::Char('l') => Some(Command::NextChar),
            _ => None,
        },
        _ => None,
    }
}

pub async fn handle_insert(
    mod_keys: &mut Vec<ModifierKeyCode>,
    input: Event
) -> Option<Command> {
    let ctrl = mod_keys.iter().find(|&k| *k == ModifierKeyCode::LeftControl || *k == ModifierKeyCode::RightControl).is_some();
    match input {
        Event::Key(key) => match key.code {
            KeyCode::Esc => Some(Command::NormalMode),

            KeyCode::Char('t') if ctrl => Some(Command::TravelMode),
            KeyCode::Char('h') if ctrl => Some(Command::PrevChar),
            KeyCode::Char('j') if ctrl => Some(Command::PrevLine),
            KeyCode::Char('k') if ctrl => Some(Command::NextLine),
            KeyCode::Char('l') if ctrl => Some(Command::NextChar),

            KeyCode::Char(ch) => Some(Command::Insert(ch)),
            _ => None,
        },
        _ => None,
    }
}

pub async fn handle_travel(
    mod_keys: &mut Vec<ModifierKeyCode>,
    input: Event
) -> Option<Command> {
    let ctrl = mod_keys.iter().find(|&k| *k == ModifierKeyCode::LeftControl || *k == ModifierKeyCode::RightControl).is_some();
    match input {
        Event::Key(key) => match key.code {
            KeyCode::Esc => Some(Command::NormalMode),

            KeyCode::Char('i') if ctrl => Some(Command::ToFirstChild),
            KeyCode::Char('k') => Some(Command::ToParent),
            KeyCode::Char('h') => Some(Command::ToLeftSibling),
            KeyCode::Char('l') => Some(Command::ToRightSibling),
            KeyCode::Char('j') => Some(Command::ToFirstChild),
            KeyCode::Char('a') => Some(Command::ToLastChild),
            KeyCode::Char('m') => Some(Command::ToMiddleChild),

            _ => None,
        },
        _ => None,
    }
}

