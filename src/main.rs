#![allow(dead_code, unused_imports)]
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver as ViewReciever};
use std::fmt::Debug;
use std::time::{Instant, Duration};
use std::{default, fs, thread, usize};

use clap::Parser;
use crossbeam_channel::{unbounded, Receiver};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use anyhow::Result;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, BorderType, Borders, List, ListDirection, Widget};
use ratatui::{
    widgets::WidgetRef,
    Frame
};

mod tui;
mod primatives;
mod zipper;
mod flipflop;

use primatives::{Layout, LayoutType, Line, Span, SplitDirection, Text};
use tui::{handle_keys, Msg};
use zipper::{Node, Zipper};

type RC<T> = Rc<RefCell<T>>;

#[derive(Parser, Debug)]
struct CLI {
    path: Option<PathBuf>,
}

#[derive(Clone)]
struct Model {
    app_state: AppState,
    layout: RC<Layout>,
}

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
enum AppState {
    #[default]
    Running,
    Stop
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

fn view(tree: Rc<RefCell<Layout>>, frame: &mut Frame) {
    let tree = tree.borrow();
    frame.render_widget(
        &*tree,
        frame.size(),
    )
}

fn handle_events(input: Event) -> Option<Msg> {
    match input {
        Event::Key(key) => handle_keys(key),
        _ => None,
    }
}

pub fn timeout_sleep(tick_rate: Duration, last_tick: Instant) -> Instant {
    let timeout = tick_rate.saturating_sub(last_tick.elapsed());
    if !timeout.is_zero() { thread::sleep(timeout); }
    Instant::now()
}

fn main() -> Result<()> {
    let path = CLI::parse().path.expect("File Required");
    let content = fs::read_to_string(path.clone()).expect("File Doesn't Exist");

    tui::install_panic_hook();
    let mut terminal = tui::init_app()?;

    let tick_rate = Duration::from_millis(16);

    let (input_tx, input_rx) = unbounded::<Event>();
    let (quit_tx, quit_rx) = unbounded::<Msg>();

    // set up and initial draw
    let model = Model {
        app_state: AppState::Running,
        layout: Rc::new(RefCell::new(Layout::new(LayoutType::Container {
            split_direction: SplitDirection::Horizontal,
            layouts: vec![
                Rc::new(RefCell::new(Layout::new(LayoutType::Content(Text::raw(content))))),
            ]
        })))
    };

    let mut zipper = Zipper::new(Node::Layout(model.layout.clone()));

    let quit_rx_input = quit_rx.clone();
    thread::spawn(move || tui::input_listener(input_tx, quit_rx_input, tick_rate));

    let mut last_tick = Instant::now();
    let mut quit = false;
    loop {
        last_tick = timeout_sleep(tick_rate, last_tick);

        terminal.draw(|f| view(model.layout.clone(), f))?;

        // handle input
        while let Ok(input) = input_rx.try_recv() {
            if let Some(msg) = handle_events(input) {
                if msg == Msg::Quit { quit = true; }
                zipper = update(zipper, msg);
            }
        }

        if quit {
            quit_tx.send(Msg::Quit).unwrap();
            break;
        }
    }

    tui::teardown_app()
}

