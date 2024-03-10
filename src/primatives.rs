use std::{cell::RefCell, cmp::min, fs, path::PathBuf, rc::Rc, str::Chars, u16};

use ratatui::{buffer::Buffer, layout::{Alignment, Rect}, style::Style, widgets::{Widget, WidgetRef}};

use crate::RC;

trait Node<T> {
    fn add_child(&mut self, child: RC<T>, index: usize);
}

//
// char
//
#[derive(Clone)]
pub struct Char {
    pub char: char,
    pub style: Style,
}

impl WidgetRef for Char {
    fn render_ref(&self,area:Rect,buf: &mut Buffer) {
        buf.set_style(area, self.style);
        buf.get_mut(area.x, area.y).set_symbol(&self.char.to_string());
    }
}

//
// span
//

#[derive(Default, Clone)]
pub struct Span {
    pub content: Vec<RC<Char>>,
    pub style: Style,
}

 impl Span { 
    pub fn raw<T: Into<String>>(content: T) -> Span {
        let content: String = content.into();
        Span {
            content: content.chars()
                .map(|ch| Rc::new(RefCell::new(
                    Char { char: ch, style: Style::default() }
                )))
                .collect(),
            style: Style::default(),
        }
    }
}

impl Node<Char> for Span {
    fn add_child(&mut self, child: RC<Char>, index: usize) {
        let len = self.content.len();
        let mut chars: Vec<RC<Char>> =
            self.content
                .drain(min(index, len)..len)
                .collect();

        self.content.push(child);
        self.content.append(&mut chars);

        self.content = chars;
    }
}

impl Node<Span> for Line {
    fn add_child(&mut self, child: RC<Span>, index: usize) {
        let len = self.spans.len();
        let mut spans: Vec<RC<Span>> =
            self.spans
                .drain(min(index, len)..len)
                .collect();

        self.spans.push(child);
        self.spans.append(&mut spans);

        self.spans = spans;
    }
}

impl Node<Line> for Text {
    fn add_child(&mut self, child: RC<Line>, index: usize) {
        let len = self.lines.len();
        let mut lines: Vec<RC<Line>> =
            self.lines
                .drain(min(index, len)..len)
                .collect();

        self.lines.push(child);
        self.lines.append(&mut lines);

        self.lines = lines;
    }
}

impl WidgetRef for Span {
    fn render_ref(&self,area:Rect,buf: &mut Buffer) {
        // height is already 1
        if self.content.len() == 0 {
            let area = Rect { width: 1, ..area };
            buf.set_style(area, self.style);
            return;
        }
        buf.set_style(area, self.style);
        let mut i: u16 = 0;
        for ch in self.content.iter() {
            let area = Rect {
                x: area.x + i,
                width: 1,
                ..area
            };
            ch.borrow().render_ref(area, buf);
            i += 1;
        }
    }
}

//
// line
//

#[derive(Default, Clone)]
pub struct Line {
    pub spans: Vec<RC<Span>>,
    pub style: Style,
    pub alignment: Option<Alignment>,
}

impl Line {
    pub fn add_span(&mut self, span: RC<Span>, index: usize) {
        let len = self.spans.len();
        let mut spans: Vec<RC<Span>> =
            self.spans
                .drain(min(index, len)..len)
                .collect();

        self.spans.push(span);
        self.spans.append(&mut spans);

        self.spans = spans;
    }

    pub fn char_len(&self) -> u16 {
        self.spans
            .iter()
            .map(|sp| sp.borrow().content.len() as u16)
            .sum()
    }
}

impl WidgetRef for Line {
    fn render_ref(&self,area:Rect,buf: &mut Buffer) {
        // height is already 1
        if self.spans.len() == 0 {
            let area = Rect { width: 1, ..area };
            buf.set_style(area, self.style);
            return;
        }
        buf.set_style(area, self.style);
        let mut offset: u16 = 0;
        for span in self.spans.iter() {
            let span = span.borrow();
            let area = Rect {
                x: area.x + offset,
                y: area.y,
                width: span.content.len() as u16,
                height: 1,
            };
            span.render_ref(area, buf);
            offset += span.content.iter().count() as u16;
        }
    }
}

//
// text
//

#[derive(Default, Clone)]
pub struct Text {
    pub lines: Vec<RC<Line>>,
    pub style: Style,
    pub alignment: Option<Alignment>,
}

impl Text {
    pub fn add_line(&mut self, line: RC<Line>, index: usize) {
        let len = self.lines.len();
        let mut lines: Vec<RC<Line>> =
            self.lines
                .drain(min(index, len)..len)
                .collect();

        self.lines.push(line);
        self.lines.append(&mut lines);

        self.lines = lines;
    }
}

impl WidgetRef for Text {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        buf.set_style(area, self.style);
        let mut line_number: u16 = 0;
        for line in self.lines.iter() {
            let area = Rect {
                x: area.x,
                y: area.y + line_number,
                width: line.borrow().char_len(),
                height: 1,
            };
            line.borrow().render_ref(area, buf);
            line_number += 1;
        }
    }
}

impl Widget for Char {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.render_ref(area, buf);
    }
}

impl Widget for Span {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.render_ref(area, buf);
    }
}

impl Widget for Line {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.render_ref(area, buf);
    }
}

impl Widget for Text {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.render_ref(area, buf);
    }
}

#[derive(Clone)]
pub enum SplitDirection {
    Vertical,
    Horizontal,
}

#[derive(Clone)]
pub enum Layout {
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

struct StatusBar {}

#[derive(Clone, Copy)]
struct EditorPosition {
    line: u32,
    character: u16,
}


#[derive(Clone)]
pub enum Content {
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
                let block = ratatui::widgets::Block::default()
                    .title(path_str)
                    .borders(ratatui::widgets::Borders::ALL)
                    .border_type(ratatui::widgets::BorderType::Rounded);

                let list = ratatui::widgets::List::new(entries.iter()
                    .map(|p| p.to_str().unwrap())
                    .collect::<Vec<&str>>()
                );

                list.block(block)
                    .direction(ratatui::widgets::ListDirection::TopToBottom)
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

