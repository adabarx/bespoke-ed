#![allow(dead_code)]
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver as ViewReciever};
use std::fmt::Debug;
use std::time::{Instant, Duration};
use std::{fs, thread, usize};

use clap::Parser;
use crossbeam_channel::{unbounded, Receiver};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use anyhow::Result;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Block, Widget};
use ratatui::{
    widgets::{Paragraph, WidgetRef},
    Frame
};

mod tui;

#[derive(Parser, Debug)]
struct CLI {
    path: Option<PathBuf>,
}

// #[derive(Clone)]
// enum WindowTree{
//     Root {
//         children: Vec<WindowTree>
//     },
//     Node {
//         // widget: Box<dyn RenderSend>,
//         children: Vec<WindowTree>
//     }
// }
//
// impl Default for WindowTree {
//     fn default() -> Self {
//         Self::Root { children: Vec::new() }
//     }
// }
//
// enum SiblingStatus {
//     Free,
//     Taken
// }
//
// struct Zipper<'a> {
//     path: Box<Option<Zipper<'a>>>,
//     focus: &'a mut WindowTree,
//     left: Vec<SiblingStatus>,
//     right: Vec<SiblingStatus>,
// }
//
// impl<'a> Zipper<'a> {
//     pub fn new(focus: &'a mut WindowTree) -> Self {
//         Self {
//             path: Box::new(None),
//             focus,
//             left: Vec::new(),
//             right: Vec::new(),
//         }
//     }
// }

#[derive(Clone)]
struct Model {
    app_state: AppState,
    window: Layout,
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

enum AppMode {
    Editor,
    FileExplorer,
}

struct StatusBar {}

#[derive(Clone)]
enum Content {
    Editor {
        lines: Vec<EditorLine>,
        // line number at top of screen
        position: u32,
    },
    FileExplorer {
        path: PathBuf,
        files: Vec<PathBuf>,
        folders: Vec<PathBuf>,
    }
}

impl Content {
    pub fn new_editor(content: String) -> Self {
        Self::Editor {
            position: 0,
            lines: content.split('\n')
                .map(|s| EditorLine { characters: s.chars().collect() })
                .collect(),
        }
    }
}

#[derive(Clone)]
struct EditorLine {
    characters: Vec<char>
}

struct App {
    mode: AppMode,
    window: Content,
}

impl Widget for Content {
    fn render(self, area: Rect, buf: &mut Buffer)
        where Self: Sized
    {
        self.render_ref(area, buf);
    }
}

impl WidgetRef for Content {
    fn render_ref(&self, area: Rect, buf: &mut Buffer)
        where Self: Sized
    {
        match self {
            Content::Editor { lines, position } => {
                for row in 0..area.height {
                    if let Some(textline) = lines.get(row as usize + *position as usize) {
                        let mut index: u16 = 0;
                        for ch in textline.characters.iter() {
                            if area.x + index > area.x + area.width { break }
                            buf.get_mut(area.x + index, area.y + row)
                                .set_symbol(&ch.to_string());
                            index += 1;
                        }
                    }
                }
            },
            Content::FileExplorer { path: _, files: _, folders: _ } => {
                // let path_str = path.to_str().unwrap();
                // Block::new()
            }
        }
    }
}

#[derive(Clone)]
enum LayoutDirection {
    Vertical,
    Horizontal,
}

#[derive(Clone)]
enum Layout {
    Nested {
        direction: LayoutDirection,
        layouts: Vec<Layout>,
    },
    Content(Content),
}

impl WidgetRef for Layout {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        match self {
            Layout::Content(content) => content.render_ref(area, buf),
            Layout::Nested { direction, layouts } => {
                let windows: u16 = layouts.len().try_into().unwrap();
                if windows == 0 { return (); }
                match direction {
                    LayoutDirection::Vertical => {
                        let offset = area.height / windows;
                        layouts.iter().enumerate().for_each(|(i, layout)| {
                            let area = Rect::new(
                                area.x,
                                if i == 0 { area.y } else { area.y + offset + 1 },
                                area.width,
                                offset
                            );
                            layout.render_ref(area, buf)
                        });
                    },
                    LayoutDirection::Horizontal => {
                        let offset = area.width / windows;
                        layouts.iter().enumerate().for_each(|(i, layout)| {
                            let area = Rect::new(
                                if i == 0 { area.x } else { area.x + offset + 1 },
                                area.y,
                                offset,
                                area.height,
                            );
                            layout.render_ref(area, buf)
                        });
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

fn view(tree: Layout, frame: &mut Frame) {
    frame.render_widget(
        &tree,
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
    view_rx: ViewReciever<Layout>,
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
    let path = CLI::parse().path.expect("File Required");
    let content = fs::read_to_string(path).expect("File Doesn't Exist");

    tui::install_panic_hook();

    let tick_rate = Duration::from_millis(16);

    let (view_tx, view_rx) = mpsc::channel::<Layout>();
    let (input_tx, input_rx) = unbounded::<Event>();
    let (quit_tx, quit_rx) = unbounded::<Msg>();

    let quit_rx_input = quit_rx.clone();
    let quit_rx_view = quit_rx.clone();
    thread::spawn(move || tui::input_listener(input_tx, quit_rx_input, tick_rate));
    thread::spawn(move || view_handler(view_rx, quit_rx_view, tick_rate));

    // set up and initial draw
    let mut model = Model {
        app_state: AppState::Running,
        window: Layout::Content(Content::new_editor(content))
    };
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

