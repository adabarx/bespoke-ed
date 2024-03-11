#![allow(dead_code, unused_imports)]
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{self, Receiver as ViewReciever};
use std::fmt::Debug;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Instant, Duration};
use std::{default, fs, thread, usize};

use clap::Parser;
use crossbeam_channel::{unbounded, Receiver};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use anyhow::Result;
use input::{handle_keys, Msg};
use ratatui::backend::Backend;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, BorderType, Borders, List, ListDirection, Widget};
use ratatui::Terminal;
use ratatui::{
    widgets::WidgetRef,
    Frame
};

mod tui;
mod primatives;
mod zipper;
mod flipflop;
mod input;

use primatives::{Layout, LayoutType, Line, Span, SplitDirection, Text};
use zipper::{Node, Zipper};

type ARW<T> = Arc<RwLock<T>>;

#[derive(Parser, Debug)]
struct CLI {
    path: Option<PathBuf>,
}

struct Model {
    app_state: Mutex<AppState>,
    layout: ARW<Layout>,
}

struct Quit;

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
enum AppState {
    #[default]
    Normal,
    Travel,
    Insert,
}

fn update(zipper: Zipper, msg: Msg) -> Zipper {
    match msg {
        Msg::ToParent => zipper.go_back_to_parent().unwrap(),
        Msg::ToFirstChild => zipper.move_to_child(0).unwrap(),
        Msg::ToLeftSibling => zipper.move_left_or_cousin().unwrap(),
        Msg::ToRightSibling => zipper.move_right_or_cousin().unwrap(),
        _ => zipper,
    }
}

fn view(tree: Arc<RwLock<Layout>>, frame: &mut Frame) {
    let tree = tree.read().unwrap();
    frame.render_widget(
        &*tree,
        frame.size(),
    )
}

pub fn timeout_sleep(tick_rate: Duration, last_tick: Instant) -> Instant {
    let timeout = tick_rate.saturating_sub(last_tick.elapsed());
    if !timeout.is_zero() { thread::sleep(timeout); }
    Instant::now()
}

fn control_loop(model: &'static RwLock<Model>, tick_rate: Duration, input_rx: Receiver<Msg>, quit_rx: Receiver<Quit>) {
    let mut zipper = Zipper::new(Node::Layout(model.read().unwrap().layout.clone()));
    let mut last_tick = Instant::now();
    loop {
        if let Ok(_) = quit_rx.try_recv() { break }

        // handle input
        while let Ok(msg) = input_rx.try_recv() {
            zipper = update(zipper, msg);
        }

        last_tick = timeout_sleep(tick_rate, last_tick);
    }
}

fn main() -> Result<()> {
    tui::install_panic_hook();

    let path = CLI::parse().path.expect("File Required");
    let content = fs::read_to_string(path.clone()).expect("File Doesn't Exist");

    // set up global model
    let model: &'static RwLock<Model> = Box::leak(Box::new(RwLock::new(
        Model {
            app_state: Mutex::new(AppState::Normal),
            layout: Arc::new(RwLock::new(Layout::new(LayoutType::Container {
                split_direction: SplitDirection::Horizontal,
                layouts: vec![
                    Arc::new(RwLock::new(Layout::new(LayoutType::Content(Text::raw(content))))),
                ]
            })))
        }
    )));

    let tick_rate = Duration::from_nanos(16_666_666);

    let (input_tx, input_rx) = unbounded::<Msg>();
    let (quit_tx, quit_rx) = unbounded::<Quit>();

    let quit_control = quit_rx.clone();

    let input_thread = thread::spawn(move || input::input_listener(input_tx, quit_tx, tick_rate));
    let control_thread = thread::spawn(move || control_loop(model, tick_rate, input_rx, quit_control));

    let mut terminal = tui::init_app()?;
    let mut last_tick = Instant::now();

    loop {
        if let Ok(_) = quit_rx.try_recv() { break }
        
        terminal.draw(|frame| {
            let tree = model.read().unwrap()
                .layout.read().unwrap()
                .clone();
            frame.render_widget_ref(tree, frame.size());
        }).unwrap();

        last_tick = timeout_sleep(tick_rate, last_tick);
    }

    tui::teardown_app()?;

    input_thread.join().unwrap().unwrap();
    control_thread.join().unwrap();

    Ok(())
}

