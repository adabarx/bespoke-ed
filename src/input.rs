use crossterm::event::{Event, KeyCode, ModifierKeyCode};
use tokio::sync::RwLock;


// pub async fn input_listener(
//     mod_keys: &'static RwLock<Vec<ModifierKeyCode>>,
//     state: &'static RwLock<State>,
//     input_tx: UnboundedSender<Msg>,
//     tick_rate: Duration,
// ) -> Result<()> {
//     let mut last_tick = Instant::now();
//     loop {
//         if *state.read().await == State::ShutDown { break }
//
//         let timeout = tick_rate.saturating_sub(last_tick.elapsed());
//         if crossterm::event::poll(timeout)? {
//             if let Some(msg) = handle_events_old(mod_keys, state, event::read()?).await {
//                 input_tx.send(msg)?;
//             }
//             last_tick = Instant::now();
//         }
//     };
//     Ok(())
// }

#[derive(PartialEq, Eq)]
pub enum Msg {
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
    _mod_keys: &'static RwLock<Vec<ModifierKeyCode>>,
    input: Event
) -> Option<Msg> {
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
            KeyCode::Esc => Some(Msg::ShutDown),
            KeyCode::Char('i') => Some(Msg::InsertMode),
            KeyCode::Char('t') => Some(Msg::TravelMode),
            KeyCode::Char('h') => Some(Msg::PrevChar),
            KeyCode::Char('j') => Some(Msg::PrevLine),
            KeyCode::Char('k') => Some(Msg::NextLine),
            KeyCode::Char('l') => Some(Msg::NextChar),
            _ => None,
        },
        _ => None,
    }
}

pub async fn handle_insert(
    mod_keys: &'static RwLock<Vec<ModifierKeyCode>>,
    input: Event
) -> Option<Msg> {
    let ctrl = mod_keys.read().await.iter().find(|&k| *k == ModifierKeyCode::LeftControl || *k == ModifierKeyCode::RightControl).is_some();
    match input {
        Event::Key(key) => match key.code {
            KeyCode::Esc => Some(Msg::NormalMode),

            KeyCode::Char('t') if ctrl => Some(Msg::TravelMode),
            KeyCode::Char('h') if ctrl => Some(Msg::PrevChar),
            KeyCode::Char('j') if ctrl => Some(Msg::PrevLine),
            KeyCode::Char('k') if ctrl => Some(Msg::NextLine),
            KeyCode::Char('l') if ctrl => Some(Msg::NextChar),

            KeyCode::Char(ch) => Some(Msg::Insert(ch)),
            _ => None,
        },
        _ => None,
    }
}

pub async fn handle_travel(
    mod_keys: &'static RwLock<Vec<ModifierKeyCode>>,
    input: Event
) -> Option<Msg> {
    let ctrl = mod_keys.read().await.iter().find(|&k| *k == ModifierKeyCode::LeftControl || *k == ModifierKeyCode::RightControl).is_some();
    match input {
        Event::Key(key) => match key.code {
            KeyCode::Esc => Some(Msg::NormalMode),

            KeyCode::Char('i') if ctrl => Some(Msg::ToFirstChild),
            KeyCode::Char('k') => Some(Msg::ToParent),
            KeyCode::Char('h') => Some(Msg::ToLeftSibling),
            KeyCode::Char('l') => Some(Msg::ToRightSibling),
            KeyCode::Char('j') => Some(Msg::ToFirstChild),
            KeyCode::Char('a') => Some(Msg::ToLastChild),
            KeyCode::Char('m') => Some(Msg::ToMiddleChild),

            _ => None,
        },
        _ => None,
    }
}

