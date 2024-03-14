use std::path::PathBuf;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::{Instant, Duration};
use std::fs;

use tokio::sync::{mpsc, RwLock};
use clap::Parser;
use crossterm::event::{self, ModifierKeyCode};
use anyhow::Result;
use input::Msg;

mod tui;
mod primatives;
mod zipper;
mod flipflop;
mod input;

use primatives::{AsyncWidget, Layout, LayoutRender, LayoutType, LayoutTypeRender, ParentWidget, SplitDirection, Text};
use tokio::time::sleep;
use zipper::RootZipper;

use crate::input::handle_events;

const BILLIE: u64 = 1_000_000_000;
const FPS_LIMIT: u64 = 60;
const RENDER_DEADLINE: u64 = BILLIE / FPS_LIMIT;
const CONTROL_DEADLINE: u64 = BILLIE / (FPS_LIMIT * 2);

type ARW<T> = Arc<RwLock<T>>;

impl AsyncWidget for &'static RwLock<Layout> {
    async fn async_render(&self) -> LayoutRender {
        let layout = self.read().await;
        LayoutRender {
            style: layout.style.clone(),
            layout: match layout.layout {
                LayoutType::Container { split_direction, layouts } => 
                    LayoutTypeRender::Container { split_direction, layouts },
                LayoutType::Content(text) =>
                    LayoutTypeRender::Content(text.async_render.await),
            }
        }
    }
}

#[derive(Parser, Debug)]
struct CLI { path: Option<PathBuf> }

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
enum State {
    #[default]
    Normal,
    Insert,
    ShutDown,
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
    let root_layout: &'static RwLock<Layout> = Box::leak(Box::new(
        RwLock::new(Layout::new(LayoutType::Container {
            split_direction: SplitDirection::Vertical,
            layouts: vec![
                Arc::new(RwLock::new(Layout::new(LayoutType::Content(Text::raw(content)))))
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

    tokio::spawn(async move {
        let tick_rate = Duration::from_nanos(CONTROL_DEADLINE);
        let mut last_tick = Instant::now();
        loop {
            if *state.read().await == State::ShutDown { break }

            let timeout = tick_rate.saturating_sub(last_tick.elapsed());
            if crossterm::event::poll(timeout).unwrap() {
                if let Some(msg) = handle_events(mod_keys, state, event::read().unwrap()) {
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

        let mut zipper = RootZipper::new(root_layout.clone()).await;
        loop {
            if *state.read().await == State::ShutDown { break }

            // handle input
            while let Ok(msg) = input_rx.try_recv() {
                // zipper = update(state, zipper, msg).await;
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
            if *state.read().await == State::ShutDown { break }
            
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

