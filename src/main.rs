#![allow(dead_code)]
use std::path::PathBuf;
use std::fmt::Debug;
use std::sync::Arc;
use std::fs;

use either::Either::Right;
use input::handle_insert;
use input::handle_normal;
use primatives::Root;
use tokio::sync::mpsc;
use tokio::sync::RwLock;
use clap::Parser;
use crossterm::event::{self, ModifierKeyCode};
use anyhow::Result;
use input::Msg;

mod tui;
mod primatives;
mod zipper;
mod flipflop;
mod input;

use primatives::{Window, WindowRender, SplitDirection, Text, AsyncWidget};
use zipper::RootZipper;
use tokio::time::{sleep, Instant, Duration};
use zipper::Zipper;

use crate::input::handle_events_old;

const BILLY: u64 = 1_000_000_000;
const FPS_LIMIT: u64 = 60;
const RENDER_DEADLINE: u64 = BILLY / FPS_LIMIT;
const CONTROL_DEADLINE: u64 = BILLY / (FPS_LIMIT * 2);

type ARW<T> = Arc<RwLock<T>>;

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

    let state: &'static RwLock<State> = Box::leak(Box::new(RwLock::new(State::Normal)));
    let mod_keys: &'static RwLock<Vec<ModifierKeyCode>> = Box::leak(Box::new(RwLock::new(Vec::new())));
    let root: &'static RwLock<Root> = Box::leak(Box::new(
        RwLock::new(Root::new(SplitDirection::Vertical))
    ));
    root.write().await.children.push(
        Arc::new(RwLock::new(Window::new(SplitDirection::Vertical)))
    );
    root.write().await.children[0].write().await.children.push(
        Right(Arc::new(RwLock::new(Text::raw(content))))
    );

    let mut terminal = tui::init_app()?;

    let (input_tx, mut input_rx) = mpsc::unbounded_channel::<Msg>();
    let (render_tx, mut render_rx) = mpsc::unbounded_channel::<WindowRender>();

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
            let timeout = tick_rate.saturating_sub(last_tick.elapsed());
            if crossterm::event::poll(timeout).unwrap() {
                // if let Some(msg) = handle_events_old(
                //     mod_keys,
                //     state,
                //     event::read().unwrap()
                // ).await {
                //     input_tx.send(msg).unwrap();
                // }
                let event = event::read().unwrap();
                let msg = match *state.read().await {
                    State::Normal => handle_normal(mod_keys, event).await,
                    State::Insert => handle_insert(mod_keys, event).await,
                    State::ShutDown => break,
                };
                if let Some(msg) = msg {
                    input_tx.send(msg).unwrap();
                }
                last_tick = Instant::now();
            }
        };
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

        let _zipper = Box::new(RootZipper::new(root).await) as Box<dyn Zipper + Send>;

        'main: loop {
            // handle input
            while let Ok(_msg) = input_rx.try_recv() {
                // execute user commands
                match *state.read().await {
                    State::Normal => (),
                    State::Insert => (),
                    State::ShutDown => break 'main,
                }
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
            
            render_tx.send(root.async_render().await).unwrap();

            last_tick = timeout_sleep(tick_rate, last_tick).await;
        }
    });

    //
    // render thread:
    //     *  stays in main thread
    //     1. receives renders from build thread
    //     2. draws the render to the terminal
    //

    // let mut fps_tick = Instant::now();
    // let mut fps = 0_f64;
    while let Some(render) = render_rx.recv().await {
        terminal.draw(|frame| frame.render_widget_ref(render, frame.size()))?;

        // rudimentary debug stuff
        //
        // let tick = Instant::now();
        // fps = 1_f64 / tick.duration_since(fps_tick).as_secs_f64();
        // fps_tick = tick;
    }

    tui::teardown_app()
}

