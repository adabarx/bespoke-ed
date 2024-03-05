#![allow(dead_code, unused_imports)]
use std::borrow::Cow;
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
use ratatui::layout::Rect;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, List, ListDirection, Widget};
use ratatui::{
    widgets::WidgetRef,
    Frame
};

mod tui;

#[derive(Parser, Debug)]
struct CLI {
    path: Option<PathBuf>,
}

#[derive(Clone)]
struct Model<'a> {
    app_state: AppState,
    layout: Rc<RefCell<Layout<'a>>>,
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

struct FileExplorer {
    path: PathBuf,
    folders: Vec<PathBuf>,
    files: Vec<PathBuf>,
    position: usize,
}

struct StatusBar {}

#[derive(Clone, Copy)]
struct EditorPosition {
    line: u32,
    character: u16,
}


#[derive(Clone)]
enum Content<'a> {
    Editor {
        text: Text<'a>,
        // line number at top of screen
        position: EditorPosition,
    },
    FileExplorer {
        path: PathBuf,
        entries: Vec<PathBuf>,
    }
}

impl<'a> Content<'a> {
    pub fn new_editor<S: Into<Cow<'a, str>>>(content: S) -> Content<'a> {
        match content.into() {
            Cow::Borrowed(c) =>
                Self::Editor {
                    position: EditorPosition { line: 0, character: 0 },
                    text: Text {
                        lines: c.split('\n')
                            .map(|line| 
                                Line {
                                    spans: line.split_inclusive(' ')
                                        .map(|s| Span::raw(s))
                                        .collect(),
                                    ..Default::default()
                                }
                            )
                            .collect(),
                        ..Default::default()
                    }
                },
            Cow::Owned(c) =>
                Self::Editor {
                    position: EditorPosition { line: 0, character: 0 },
                    text: Text {
                        lines: c.split('\n')
                            .map(|line| 
                                Line {
                                    spans: line.split_inclusive(' ')
                                        .map(|s| Span::raw(s.to_owned()))
                                        .collect(),
                                    ..Default::default()
                                }
                            )
                            .collect(),
                        ..Default::default()
                    }
                },
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

impl WidgetRef for Content<'_> {
    fn render_ref(&self, area: Rect, buf: &mut Buffer)
        where Self: Sized
    {
        match self {
            Content::Editor { text, position: _ } => text.render_ref(area, buf),
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

#[derive(Clone)]
struct EditorLine {
    characters: Vec<char>
}

impl Widget for Content<'_> {
    fn render(self, area: Rect, buf: &mut Buffer)
        where Self: Sized
    {
        self.render_ref(area, buf);
    }
}

enum ZipperMoveResult<'a> {
    Success(Zipper<'a>),
    Failed(Zipper<'a>)
}

type LayoutPointer<'a> = Rc<RefCell<Layout<'a>>>;

enum PrevDir {
    Parent,
    Left,
    Right,
}

struct Breadcrumb<'a> {
    zipper: Box<Zipper<'a>>,
    direction: PrevDir,
}

struct Zipper<'a> {
    previous: Option<Breadcrumb<'a>>,
    focus: LayoutPointer<'a>,
    children: Vec<LayoutPointer<'a>>,
    left: Vec<LayoutPointer<'a>>,
    right: Vec<LayoutPointer<'a>>
}

impl<'a> Zipper<'a> {
    pub fn move_to_child(self, index: usize) -> ZipperMoveResult<'a> {
        if index >= self.children.len() { return ZipperMoveResult::Failed(self) }
        let left = self.children[0..index].iter().cloned().collect();
        let right = self.children[index + 1..self.children.len()].iter().cloned().collect();
        let focus = self.children[index].clone();
        let children = match &*focus.borrow() {
            Layout::Content(_) => Vec::new(),
            Layout::Container { layouts, .. } => layouts.iter().cloned().collect(),
        };
        let previous = Some(Breadcrumb { zipper: Box::new(self), direction: PrevDir::Parent });

        ZipperMoveResult::Success(Zipper { focus, previous, children, left, right })
    }

    pub fn move_left(self) -> ZipperMoveResult<'a> {
        if self.left.len() == 0  { return ZipperMoveResult::Failed(self) }
        let mut left = self.left.clone();
        let focus = left.pop().unwrap();
        let mut tmp_right = self.right.clone();
        let mut right = vec![self.focus.clone()];
        right.append(&mut tmp_right);
        let children = match &*focus.borrow() {
            Layout::Content(_) => Vec::new(),
            Layout::Container { layouts, .. } => layouts.iter().cloned().collect(),
        };
        let previous = Some(Breadcrumb { zipper: Box::new(self), direction: PrevDir::Right });

        ZipperMoveResult::Success(Zipper { focus, previous, children, left, right })
    }

    pub fn move_right(self) -> ZipperMoveResult<'a> {
        if self.right.len() == 0  { return ZipperMoveResult::Failed(self) }
        let right: Vec<LayoutPointer> = self.right[1..].iter().cloned().collect();
        let focus = right[0].clone();
        let mut left = self.left.clone();
        left.push(self.focus.clone());
        let children = match &*focus.borrow() {
            Layout::Content(_) => Vec::new(),
            Layout::Container { layouts, .. } => layouts.iter().cloned().collect(),
        };
        let previous = Some(Breadcrumb { zipper: Box::new(self), direction: PrevDir::Left });

        ZipperMoveResult::Success(Zipper { focus, previous, children, left, right })
    }

    pub fn track_back_to_parent(self) -> ZipperMoveResult<'a> {
        if let Some(zip) = self.previous {
            match zip.direction {
                PrevDir::Parent => ZipperMoveResult::Success(*zip.zipper),
                _ => {
                    let crumb = match zip.zipper.track_back_to_parent() {
                        ZipperMoveResult::Success(z) => z,
                        ZipperMoveResult::Failed(_) =>
                            panic!("zipper.move_to_parent shouldn't be able to fail here"),
                    };
                    
                    ZipperMoveResult::Success(crumb)
                }
            }
        } else {
            ZipperMoveResult::Failed(self)
        }
    }
}

#[derive(Clone)]
enum SplitDirection {
    Vertical,
    Horizontal,
}

#[derive(Clone)]
enum Layout<'a> {
    Container {
        split_direction: SplitDirection,
        layouts: Vec<Rc<RefCell<Layout<'a>>>>,
    },
    Content(Content<'a>),
}

impl<'a> WidgetRef for Layout<'a> {
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

fn update(mut model: Model, msg: Msg) -> Model{
    match msg {
        Msg::Quit => model.app_state = AppState::Stop,
        _ => ()
    }
    model
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
    let mut model = Model {
        app_state: AppState::Running,
        layout: Rc::new(RefCell::new(Layout::Container {
            split_direction: SplitDirection::Horizontal,
            layouts: vec![
                Rc::new(RefCell::new(Layout::Content(Content::new_editor(content)))),
                Rc::new(RefCell::new(Layout::Content(Content::new_file_explorer(path.parent().unwrap().to_path_buf()))))
            ]
        }))
    };

    let quit_rx_input = quit_rx.clone();
    thread::spawn(move || tui::input_listener(input_tx, quit_rx_input, tick_rate));

    let mut last_tick = Instant::now();
    loop {
        last_tick = timeout_sleep(tick_rate, last_tick);

        terminal.draw(|f| view(model.layout.clone(), f))?;

        // handle input
        while let Ok(input) = input_rx.try_recv() {
            if let Some(msg) = handle_events(input) {
                model = update(model, msg);
            }
        }

        if model.app_state == AppState::Stop {
            quit_tx.send(Msg::Quit)?;
            break;
        }
    }

    tui::teardown_app()
}

