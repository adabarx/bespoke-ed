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

use primatives::{Char, Line, Span, Text};
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

#[derive(PartialEq, Eq)]
pub enum Msg {
    ToFirstChild,
    ToParent,
    ToLeftSibling,
    ToRightSibling,
    Reset,
    Quit
}

struct StatusBar {}

#[derive(Clone, Copy)]
struct EditorPosition {
    line: u32,
    character: u16,
}


#[derive(Clone)]
enum Content {
    Editor {
        text: Text,
        // line number at top of screen
        position: EditorPosition,
    },
    FileExplorer {
        path: PathBuf,
        entries: Vec<PathBuf>,
    }
}

impl Content {
    pub fn new_editor<S: Into<String>>(content: S) -> Content {
        let c: String = content.into();
        Self::Editor {
            position: EditorPosition { line: 0, character: 0 },
            text: Text {
                lines: c.split('\n')
                    .map(|line| 
                        Rc::new(RefCell::new(Line {
                            spans: line.split_inclusive(' ')
                                .map(|s| Rc::new(RefCell::new(Span::raw(s))))
                                .collect(),
                            ..Default::default()
                        }))
                    )
                    .collect(),
                ..Default::default()
            }
        }
    }

    pub fn new_file_explorer(path: PathBuf) -> Self {
        let entries = fs::read_dir(path.clone())
            .unwrap()
            .filter_map(|dir| {
                let d = dir.ok()?;
                Some(d.path())
            })
            .collect();

        Self::FileExplorer { path, entries }
    }
}

impl WidgetRef for Content {
    fn render_ref(&self, area: Rect, buf: &mut Buffer)
        where Self: Sized
    {
        match self {
            Content::Editor { text, .. } => text.render_ref(area, buf),
            Content::FileExplorer { path, entries } => {
                let path_str = path.to_str().unwrap();
                let block = Block::default()
                    .title(path_str)
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded);

                let list = List::new(entries.iter()
                    .map(|p| p.to_str().unwrap())
                    .collect::<Vec<&str>>()
                );

                list.block(block)
                    .direction(ListDirection::TopToBottom)
                    .render(area, buf);
            }
        }
    }
}

impl Widget for Content {
    fn render(self, area: Rect, buf: &mut Buffer)
        where Self: Sized
    {
        self.render_ref(area, buf);
    }
}

#[derive(Clone)]
enum SplitDirection {
    Vertical,
    Horizontal,
}

#[derive(Clone)]
enum Layout {
    Container {
        split_direction: SplitDirection,
        layouts: Vec<RC<Layout>>,
    },
    Content(Content),
}

impl<'a> WidgetRef for Layout {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        match self {
            Layout::Content(content) => content.render_ref(area, buf),
            Layout::Container { split_direction, layouts } => {
                let windows: u16 = layouts.len().try_into().unwrap();
                if windows == 0 { return (); }
                match split_direction {
                    SplitDirection::Horizontal => {
                        // split is horizontal. nested containers are stacked vertically
                        let offset = area.height / windows;
                        for (i, layout) in layouts.iter().cloned().enumerate() {
                            let area = Rect::new(
                                area.x,
                                if i == 0 { area.y } else { area.y + offset + 1 },
                                area.width,
                                offset
                            );
                            layout.borrow().render_ref(area, buf)
                        }
                    },
                    SplitDirection::Vertical => {
                        // split is vertical. nested containers are stacked horizontally
                        let offset = area.width / windows;
                        for (i, layout) in layouts.iter().enumerate() {
                            let area = Rect::new(
                                if i == 0 { area.x } else { area.x + offset + 1 },
                                area.y,
                                offset,
                                area.height,
                            );
                            layout.borrow().render_ref(area, buf)
                        }
                    },
                }
            },
        }
    }
}

fn update(zipper: Zipper, msg: Msg) -> Zipper {
    match msg {
        Msg::ToParent => zipper.track_back_to_parent().unwrap(),
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

fn handle_keys(key: KeyEvent) -> Option<Msg> {
    match key.kind {
        KeyEventKind::Press => match key.code {
            KeyCode::Char('j') => Some(Msg::ToFirstChild),
            KeyCode::Char('k') => Some(Msg::ToParent),
            KeyCode::Char('h') => Some(Msg::ToLeftSibling),
            KeyCode::Char('l') => Some(Msg::ToRightSibling),
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
        layout: Rc::new(RefCell::new(Layout::Container {
            split_direction: SplitDirection::Horizontal,
            layouts: vec![
                Rc::new(RefCell::new(Layout::Content(Content::new_editor(content)))),
            ]
        }))
    };

    let mut zipper = Zipper::new(Node::Layout(model.layout.clone()));

    let quit_rx_input = quit_rx.clone();
    thread::spawn(move || tui::input_listener(input_tx, quit_rx_input, tick_rate));

    let mut last_tick = Instant::now();
    let mut quit = false;
    'main: loop {
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
            break 'main;
        }
    }

    tui::teardown_app()
}

