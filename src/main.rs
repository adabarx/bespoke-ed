#![allow(dead_code)]
use std::fmt::Debug;
use std::rc::Rc;
use std::time::{Instant, Duration};
use std::thread;

use crossbeam_channel::{unbounded, Receiver};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use anyhow::Result;
use dyn_clone::DynClone;
use ratatui::{
    widgets::{Paragraph, Widget},
    Frame
};

mod tui;

trait RenderSend: Widget + DynClone + Send + Sync {}

impl Clone for Box<dyn RenderSend> {
    fn clone(&self) -> Self {
        dyn_clone::clone_box(&**self)
    }
}

enum WidgetTree {
    Root {
        children: Vec<WidgetTree>
    },
    Node {
        widget: Box<dyn RenderSend>,
        children: Vec<WidgetTree>
    }
}

#[derive(Clone)]
enum WindowTree{
    Root {
        children: Vec<Rc<WindowTree>>
    },
    Node {
        widget: Box<dyn RenderSend>,
        children: Vec<Rc<WindowTree>>
    }
}

impl WindowTree {
    pub fn children(&self) -> Vec<Rc<WindowTree>> {
        match self {
            WindowTree::Root { children } => children.iter().cloned().collect(),
            WindowTree::Node { children, .. } => children.iter().cloned().collect(),
        }
    }

    pub fn send(&self) -> WidgetTree {
        match self {
            WindowTree::Root { children } => {
                WidgetTree::Root { 
                    children: children.iter()
                        .map(|c| (*c).send())
                        .collect()
                }
            },
            WindowTree::Node { children, widget } => {
                WidgetTree::Node {
                    widget: widget.clone(),
                    children: children.iter()
                        .map(|c| (*c).send())
                        .collect()
                }
            },
        }
    }
}

impl Default for WindowTree {
    fn default() -> Self {
        Self::Root { children: Vec::new() }
    }
}

struct Zipper {
    path: Box<Option<Zipper>>,
    focus: Rc<WindowTree>,
    left: Vec<Rc<WindowTree>>,
    right: Vec<Rc<WindowTree>>,
    children: Vec<Rc<WindowTree>>
}

impl Zipper {
    pub fn new(window: Rc<WindowTree>) -> Self {
        Self {
            path: Box::new(None),
            focus: window.clone(),
            left: Vec::new(),
            right: Vec::new(),
            children: window.children()
        }
    }

    pub fn focus_child(self, index: usize) -> Option<Zipper> {
        if index >= self.children.len() { return None };

        let left = self.children[0..index].iter().cloned().collect();
        let right = self.children[index + 1..].iter().cloned().collect();
        let focus = self.children[index].clone();
        let children = focus.children();

        Some(Zipper { path: Box::new(Some(self)), focus, left, right, children })
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

fn update(mut model: Model, msg: Msg) -> Model{
    match msg {
        Msg::Increment => model.counter += 1,
        Msg::Decrement => model.counter -= 1,
        Msg::Reset => model.counter = 0,
        Msg::Quit => model.app_state = AppState::Stop,
    }
    model
}

fn view(_tree: WidgetTree, frame: &mut Frame) {
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
    view_rx: Receiver<WidgetTree>,
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

    let (view_tx, view_rx) = unbounded::<WidgetTree>();
    let (input_tx, input_rx) = unbounded::<Event>();
    let (quit_tx, quit_rx) = unbounded::<Msg>();

    let quit_rx_input = quit_rx.clone();
    let quit_rx_view = quit_rx.clone();
    thread::spawn(move || tui::input_listener(input_tx, quit_rx_input, tick_rate));
    thread::spawn(move || view_handler(view_rx, quit_rx_view, tick_rate));

    // set up and initial draw
    let mut model = Model::default();
    view_tx.send(model.window.send())?;

    let mut last_tick = Instant::now();
    loop {
        last_tick = timeout_sleep(tick_rate, last_tick);

        // handle input
        while let Ok(input) = input_rx.try_recv() {
            if let Some(msg) = handle_events(input) {
                model = update(model, msg);
                view_tx.send(model.window.send())?;
            }
        }

        if model.app_state == AppState::Stop {
            quit_tx.send(Msg::Quit)?;
            break;
        }
    }

    tui::teardown_app()
}

