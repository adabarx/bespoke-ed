use std::path::PathBuf;
use std::fmt::Debug;
use std::sync::{RwLock, Arc};
use std::time::{Instant, Duration};
use std::{fs, thread};

use tokio::sync::mpsc;
use clap::Parser;
use crossterm::event::{self, ModifierKeyCode};
use anyhow::Result;
use input::{InsertCommand, Msg, NormalCommand};

mod tui;
mod primatives;
mod zipper;
mod flipflop;
mod input;

use primatives::{Layout, LayoutRender, LayoutType, SplitDirection, Text, AsyncWidget};
use tokio::time::sleep;
use zipper::Zipper;

use crate::input::handle_events;

const BILLY: u64 = 1_000_000_000;
const FPS_LIMIT: u64 = 60;
const RENDER_DEADLINE: u64 = BILLY / FPS_LIMIT;
const CONTROL_DEADLINE: u64 = BILLY / (FPS_LIMIT * 2);

type InputRW<T> = Arc<RwLock<T>>;
type TokioRW<T> = Arc<tokio::sync::RwLock<T>>;


#[derive(Parser, Debug)]
struct CLI { path: Option<PathBuf> }

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
enum State {
    #[default]
    Normal,
    Insert,
    ShutDown,
}

async fn update(
    state: &'static RwLock<State>,
    zipper: impl Zipper<Layout>,
    msg: Msg
) -> impl Zipper<Layout> {
    match msg {
        Msg::Normal(nc) => update_normal(state, zipper, nc).await,
        Msg::Insert(ic) => update_insert(state, zipper, ic).await,
        _ => zipper,
    }
}


async fn update_normal(
    state: &'static RwLock<State>,
    mut zipper: impl Zipper<Layout>,
    msg: NormalCommand
) -> OldZipper {
    match msg {
        NormalCommand::Quit => *state.write().await = State::ShutDown,
        NormalCommand::NextChar => zipper = zipper.move_right_or_cousin().await.unwrap(),
        NormalCommand::PrevChar => zipper = zipper.move_left_or_cousin().await.unwrap(),
        NormalCommand::PrevLine => {
            zipper = zipper
                .go_back_to_parent().await.unwrap()
                .go_back_to_parent().await.unwrap()
                .move_left_catch_ignore().await
                .move_to_child(0).await.unwrap()
                .move_to_child(0).await.unwrap();
        },
        NormalCommand::NextLine => {
            zipper = zipper
                .go_back_to_parent().await.unwrap()
                .go_back_to_parent().await.unwrap()
                .move_right_catch_ignore().await
                .move_to_child(0).await.unwrap()
                .move_to_child(0).await.unwrap();
        },
        NormalCommand::InsertMode => *state.write().await = State::Insert,
        NormalCommand::InsertModeAfterCursor => {
            zipper = zipper.move_right_or_cousin().await.unwrap();
            *state.write().await = State::Insert;
        },
    };
    zipper
}

async fn update_insert(
    state: &'static RwLock<State>,
    zipper: OldZipper,
    msg: InsertCommand
) -> OldZipper {
    match msg {
        InsertCommand::Insert(_ch) => (),
        InsertCommand::Replace(_ch) => (),
        InsertCommand::Delete => (),
        InsertCommand::Backspace => (),
        InsertCommand::NewLine => (),
        InsertCommand::NewLineBefore => (),
        InsertCommand::Normal => *state.write().await = State::Normal,
    }
    zipper
}

pub async fn timeout_sleep(tick_rate: Duration, last_tick: Instant) -> Instant {
    let timeout = tick_rate.saturating_sub(last_tick.elapsed());
    if !timeout.is_zero() { sleep(timeout).await; }
    Instant::now()
}

#[tokio::main]
async fn main() -> Result<()> {
    tui::install_panic_hook();

    let path = CLI::parse().path.expect("File Required");
    let content = fs::read_to_string(path.clone()).expect("File Doesn't Exist");

    // set up global model
    let state: &'static RwLock<State> = Box::leak(Box::new(RwLock::new(State::Normal)));
    let mod_keys: &'static RwLock<Vec<ModifierKeyCode>> = Box::leak(Box::new(RwLock::new(Vec::new())));
    let root_layout: &'static tokio::sync::RwLock<Layout> = Box::leak(Box::new(
        tokio::sync::RwLock::new(Layout::new(LayoutType::Container {
            split_direction: SplitDirection::Vertical,
            layouts: vec![
                Arc::new(tokio::sync::RwLock::new(Layout::new(LayoutType::Content(Text::raw(content)))))
            ]
        }))
    ));

    let mut terminal = tui::init_app()?;

    let (input_tx, mut input_rx) = mpsc::unbounded_channel::<Msg>();
    let (render_tx, mut render_rx) = mpsc::unbounded_channel::<LayoutRender>();

    //
    // input thread:
    //     1. handles all input from the terminal
    //     2. converts them into commands
    //     3. sends the commands to the control thread
    //

    thread::spawn(move || -> Result<()> {
        let tick_rate = Duration::from_nanos(CONTROL_DEADLINE);
        let mut last_tick = Instant::now();
        loop {
            if *state.read().unwrap() == State::ShutDown { break }

            let timeout = tick_rate.saturating_sub(last_tick.elapsed());
            if crossterm::event::poll(timeout).unwrap() {
                if let Some(msg) = handle_events(mod_keys, state, event::read()?) {
                    input_tx.send(msg).unwrap();
                }
                last_tick = Instant::now();
            }
        };
        Ok(())
    });

    //
    // control thread:
    //     1. receives commands from input thread
    //     2. executes users commands through zippers
    //     3. zippers modify the atomic tree
    //

    tokio::spawn(async move {
        let tick_rate = Duration::from_nanos(CONTROL_DEADLINE);
        let mut last_tick = Instant::now();

        let mut zipper = OldZipper::new(Node::Layout(root_layout.clone())).await;
        loop {
            if *state.read().unwrap() == State::ShutDown { break }

            // handle input
            while let Ok(msg) = input_rx.try_recv() {
                zipper = update(state, zipper, msg).await;
            }

            last_tick = timeout_sleep(tick_rate, last_tick).await;
        }
    });

    //
    // build thread:
    //     1. asynchronously traverses the atomic tree
    //     2. builds a render of the current state of the atomic tree
    //     3. sends the render to the render thread
    //

    tokio::spawn(async move {
        let tick_rate = Duration::from_nanos(RENDER_DEADLINE);
        let mut last_tick = Instant::now();
        loop {
            if *state.read().unwrap() == State::ShutDown { break }
            
            render_tx.send(root_layout.read().await.async_render().await).unwrap();

            last_tick = timeout_sleep(tick_rate, last_tick).await;
        }
    });

    //
    // render thread:
    //     *  stays in main thread
    //     1. receives renders from build thread
    //     2. draws the render to the terminal
    //

    while let Some(render) = render_rx.recv().await {
        terminal.draw(|frame| frame.render_widget_ref(render, frame.size()))?;
    }

    tui::teardown_app()
}

