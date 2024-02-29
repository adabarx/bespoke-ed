#![allow(dead_code)]
use std::sync::mpsc::{self, Receiver as ViewReciever};
use std::fmt::Debug;
use std::time::{Instant, Duration};
use std::{thread, usize};

use crossbeam_channel::{unbounded, Receiver};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use anyhow::Result;
use dyn_clone::DynClone;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::{
    widgets::{Paragraph, Widget},
    Frame
};

mod tui;

trait RenderSend: Widget + DynClone + Send {}

impl Clone for Box<dyn RenderSend> {
    fn clone(&self) -> Self {
        dyn_clone::clone_box(&**self)
    }
}

#[derive(Clone)]
enum WindowTree{
    Root {
        children: Vec<WindowTree>
    },
    Node {
        widget: Box<dyn RenderSend>,
        children: Vec<WindowTree>
    }
}

impl WindowTree {
    pub fn children_refs(&self) -> Vec<&WindowTree> {
        match self {
            WindowTree::Root { children } => children.iter().collect(),
            WindowTree::Node { children, .. } => children.iter().collect(),
        }
    }
}

impl Default for WindowTree {
    fn default() -> Self {
        Self::Root { children: Vec::new() }
    }
}

enum SiblingStatus {
    Free,
    Taken
}

struct Zipper<'a> {
    path: Box<Option<Zipper<'a>>>,
    focus: &'a mut WindowTree,
    left: Vec<SiblingStatus>,
    right: Vec<SiblingStatus>,
}

enum ZipperMoveResult<'a> {
    Success(Zipper<'a>),
    Nil(Zipper<'a>)
}

impl<'a> Zipper<'a> {
    pub fn new(focus: &'a mut WindowTree) -> Self {
        Self {
            path: Box::new(None),
            focus,
            left: Vec::new(),
            right: Vec::new(),
        }
    }
}

#[derive(Default, Clone)]
struct Model {
    counter: isize,
    app_state: AppState,
    window: WindowTree,
}

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
enum AppState {
    #[default]
    Running,
    Stop
}

pub enum Msg {
    Increment,
    Decrement,
    Reset,
    Quit
}

struct FileExplorer; // this is next

struct EditorWindow {
    lines: Vec<EditorLine>,
    // line number at top of screen
    position: usize,
}

struct EditorLine {
    characters: Vec<char>
}

impl Widget for EditorWindow {
    fn render(self, area: Rect, buf: &mut Buffer)
        where Self: Sized
    {
        for row in 0..area.height {
            if let Some(textline) = self.lines.get(row as usize + self.position) {
                let mut index: u16 = 0;
                for ch in textline.characters.iter() {
                    if area.x + index > area.x + area.width { break }
                    buf.get_mut(area.x + index, area.y + row)
                        .set_symbol(&ch.to_string());
                    index += 1;
                }
            }
        }
    }
}

fn update(mut model: Model, msg: Msg) -> Model{
    match msg {
        Msg::Increment => model.counter += 1,
        Msg::Decrement => model.counter -= 1,
        Msg::Reset => model.counter = 0,
        Msg::Quit => model.app_state = AppState::Stop,
    }
    model
}

fn view(_tree: WindowTree, frame: &mut Frame) {
    frame.render_widget(
        Paragraph::new(format!("Counter: ")),
        frame.size(),
    )
}

fn handle_keys(key: KeyEvent) -> Option<Msg> {
    match key.kind {
        KeyEventKind::Press => match key.code {
            KeyCode::Char('j') => Some(Msg::Increment),
            KeyCode::Char('k') => Some(Msg::Decrement),
            KeyCode::Char('r') => Some(Msg::Reset),
            KeyCode::Char('q') => Some(Msg::Quit),
            _ => None,
        },
        KeyEventKind::Repeat => None,
        KeyEventKind::Release => None,
    }
}

fn handle_events(input: Event) -> Option<Msg> {
    match input {
        Event::Key(key) => handle_keys(key),
        _ => None,
    }
}

fn view_handler(
    view_rx: ViewReciever<WindowTree>,
    quit_rx: Receiver<Msg>,
    tick_rate: Duration,
) -> Result<()> {
    let mut terminal = tui::init_app()?;
    let mut last_tick = Instant::now();
    loop {
        if quit_rx.try_recv().is_ok() { break }

        last_tick = timeout_sleep(tick_rate, last_tick);

        // draw
        while let Ok(model) = view_rx.try_recv() {
            terminal.draw(|f| view(model, f))?;
        }
    }
    tui::teardown_app()?;
    Ok(())
}

pub fn timeout_sleep(tick_rate: Duration, last_tick: Instant) -> Instant {
    let timeout = tick_rate.saturating_sub(last_tick.elapsed());
    if !timeout.is_zero() { thread::sleep(timeout); }
    Instant::now()
}

fn main() -> Result<()> {
    tui::install_panic_hook();

    let tick_rate = Duration::from_millis(16);

    let (view_tx, view_rx) = mpsc::channel::<WindowTree>();
    let (input_tx, input_rx) = unbounded::<Event>();
    let (quit_tx, quit_rx) = unbounded::<Msg>();

    let quit_rx_input = quit_rx.clone();
    let quit_rx_view = quit_rx.clone();
    thread::spawn(move || tui::input_listener(input_tx, quit_rx_input, tick_rate));
    thread::spawn(move || view_handler(view_rx, quit_rx_view, tick_rate));

    // set up and initial draw
    let mut model = Model::default();
    view_tx.send(model.window.clone()).unwrap();

    let mut last_tick = Instant::now();
    loop {
        last_tick = timeout_sleep(tick_rate, last_tick);

        // handle input
        while let Ok(input) = input_rx.try_recv() {
            if let Some(msg) = handle_events(input) {
                model = update(model, msg);
                view_tx.send(model.window.clone()).unwrap();
            }
        }

        if model.app_state == AppState::Stop {
            quit_tx.send(Msg::Quit)?;
            break;
        }
    }

    tui::teardown_app()
}

