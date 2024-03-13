use std::path::PathBuf;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::{Instant, Duration};
use std::fs;

use tokio::sync::mpsc::{self, Receiver, UnboundedReceiver};
use tokio::sync::RwLock;
use clap::Parser;
use crossterm::event::ModifierKeyCode;
use anyhow::Result;
use input::{InsertCommand, Msg, NormalCommand};

mod tui;
mod primatives;
mod zipper;
mod flipflop;
mod input;

use primatives::{Layout, LayoutType, SplitDirection, Text};
use tokio::time::sleep;
use zipper::{LayoutZipper, Node};

type ARW<T> = Arc<RwLock<T>>;

#[derive(Parser, Debug)]
struct CLI {
    path: Option<PathBuf>,
}

struct Model {
    state: ARW<State>,
    mod_keys: ARW<Vec<ModifierKeyCode>>,
    layout: ARW<Layout>,
}

struct Quit;

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
enum State {
    #[default]
    Normal,
    Insert,
    ShutDown,
}

async fn update(
    model: &'static Model,
    zipper: LayoutZipper,
    msg: Msg
) -> LayoutZipper {
    match msg {
        Msg::Normal(nc) => update_normal(model, zipper, nc).await,
        Msg::Insert(ic) => update_insert(model, zipper, ic).await,
        // Msg::ToParent => zipper.go_back_to_parent().unwrap(),
        // Msg::ToFirstChild => zipper.move_to_child(0).unwrap(),
        // Msg::ToLeftSibling => zipper.move_left_or_cousin().unwrap(),
        // Msg::ToRightSibling => zipper.move_right_or_cousin().unwrap(),
        _ => zipper,
    }
}

async fn update_normal(
    model: &'static Model,
    mut zipper: LayoutZipper,
    msg: NormalCommand
) -> LayoutZipper {
    match msg {
        NormalCommand::Quit => *model.state.write().await = State::ShutDown,
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
        NormalCommand::InsertMode => *model.state.write().await = State::Insert,
        NormalCommand::InsertModeAfterCursor => {
            zipper = zipper.move_right_or_cousin().await.unwrap();
            *model.state.write().await = State::Insert;
        },
    };
    zipper
}

async fn update_insert(model: &'static Model, zipper: LayoutZipper, msg: InsertCommand) -> LayoutZipper {
    match msg {
        InsertCommand::Insert(_ch) => (),
        InsertCommand::Replace(_ch) => (),
        InsertCommand::Delete => (),
        InsertCommand::Backspace => (),
        InsertCommand::NewLine => (),
        InsertCommand::NewLineBefore => (),
        InsertCommand::Normal => *model.state.write().await = State::Normal,
    }
    zipper
}

pub async fn timeout_sleep(tick_rate: Duration, last_tick: Instant) -> Instant {
    let timeout = tick_rate.saturating_sub(last_tick.elapsed());
    if !timeout.is_zero() { sleep(timeout).await; }
    Instant::now()
}

async fn control_loop(model: &'static Model, tick_rate: Duration, mut input_rx: UnboundedReceiver<Msg>) {
    let mut zipper = LayoutZipper::new(Node::Layout(model.layout.clone())).await;
    let mut last_tick = Instant::now();
    loop {
        if *model.state.read().await == State::ShutDown { break }

        // handle input
        while let Ok(msg) = input_rx.try_recv() {
            zipper = update(model, zipper, msg).await;
        }

        last_tick = timeout_sleep(tick_rate, last_tick).await;
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tui::install_panic_hook();

    let path = CLI::parse().path.expect("File Required");
    let content = fs::read_to_string(path.clone()).expect("File Doesn't Exist");

    // set up global model
    let model: &'static Model = Box::leak(Box::new(
        Model {
            state: Arc::new(RwLock::new(State::Normal)),
            mod_keys: Arc::new(RwLock::new(Vec::new())),
            layout: Arc::new(RwLock::new(Layout::new(LayoutType::Container {
                split_direction: SplitDirection::Horizontal,
                layouts: vec![
                    Arc::new(RwLock::new(Layout::new(LayoutType::Content(Text::raw(content))))),
                ]
            })))
        }
    ));

    let tick_rate = Duration::from_nanos(16_666_666);

    let (input_tx, input_rx) = mpsc::unbounded_channel::<Msg>();

    tokio::spawn(async move { input::input_listener(model, input_tx, tick_rate) });
    tokio::spawn(async move { control_loop(model, tick_rate, input_rx) });

    let mut terminal = tui::init_app()?;
    let mut last_tick = Instant::now();

    loop {
        if *model.state.read().await == State::ShutDown { break }
        
        let tree = model.layout.read().await.clone();
        terminal.draw(|frame| frame.render_widget_ref(tree, frame.size())).unwrap();

        last_tick = timeout_sleep(tick_rate, last_tick).await;
    }

    tui::teardown_app()
}

