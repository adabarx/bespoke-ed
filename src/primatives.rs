use std::{cell::{Ref, RefCell}, cmp::min, fs, path::PathBuf, rc::Rc, str::Chars, sync::{Arc, RwLock}, u16};

use anyhow::{bail, Result};
use ratatui::{buffer::Buffer, layout::{Alignment, Rect}, style::Style, widgets::{Widget, WidgetRef}};

use crate::ARW;

pub trait Mother<T> {
    fn add_child(&mut self, child: T, index: usize) -> ARW<T>;
}

pub trait TryMother<T> {
    fn try_add_child(&mut self, child: T, index: usize) -> Result<ARW<T>>;
}

#[derive(Default, Clone)]
pub struct Char {
    pub char: char,
    pub style: Style,
}

#[derive(Default, Clone)]
pub struct Span {
    pub characters: Vec<ARW<Char>>,
    pub style: Style,
}

#[derive(Default, Clone)]
pub struct Line {
    pub spans: Vec<ARW<Span>>,
    pub style: Style,
}

#[derive(Default, Clone)]
pub struct Text {
    pub lines: Vec<ARW<Line>>,
    pub style: Style,
    pub alignment: Option<Alignment>,
}

#[derive(Clone)]
pub enum SplitDirection {
    Vertical,
    Horizontal,
}

#[derive(Clone)]
pub struct Layout {
    pub style: Style,
    pub layout: LayoutType,
}

#[derive(Clone)]
pub enum LayoutType {
    Container {
        split_direction: SplitDirection,
        layouts: Vec<ARW<Layout>>,
    },
    Content(Text),
}

impl Layout {
    pub fn new(layout: LayoutType) -> Self {
        Self {
            layout,
            style: Style::default()
        }
    }
}

impl Span { 
    pub fn raw<T: Into<String>>(content: T) -> Span {
        let content: String = content.into();
        Span {
            characters: content
                .chars()
                .map(|ch| Arc::new(RwLock::new(
                    Char { char: ch, style: Style::default() }
                )))
                .collect(),
            style: Style::default(),
        }
    }

    pub fn is_newline(&self) -> bool {
        if self.characters.len() == 1
            && self.characters[0].read().unwrap().char == b'\n' as char
        {
            return true;
        }
        false

    }
}

impl Text {
    pub fn raw(input: String) -> Text {
        Text {
            lines: input
                .split_inclusive('\n')
                .map(|ln| Arc::new(RwLock::new(Line::raw(ln))))
                .collect(),
            ..Default::default()
        }
    }

    pub fn add_line(&mut self, line: ARW<Line>, index: usize) {
        let len = self.lines.len();
        let mut lines: Vec<ARW<Line>> =
            self.lines
                .drain(min(index, len)..len)
                .collect();

        self.lines.push(line);
        self.lines.append(&mut lines);

        self.lines = lines;
    }

    pub fn get_line(&self, index: usize) -> ARW<Line> {
        self.lines.get(index)
            .unwrap_or(
                self.lines.get(self.lines.len() - 1).unwrap()
            ).clone()
    }
}

impl Line {
    pub fn raw<T: Into<String>>(input: T) -> Line {
        let spans: String = input.into();
        Line {
            spans: spans
                .split_inclusive(' ')
                .map(|sp| Arc::new(RwLock::new(Span::raw(sp))))
                .collect(),
            ..Default::default()
        }
    }
    pub fn add_span(&mut self, span: ARW<Span>, index: usize) {
        let len = self.spans.len();
        let mut spans: Vec<ARW<Span>> =
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
            .map(|sp| sp.read().unwrap().characters.len() as u16)
            .sum()
    }
}

impl TryMother<Line> for Layout {
    fn try_add_child(&mut self, child: Line, index: usize) -> Result<ARW<Line>> {
        Ok(match self.layout {
            LayoutType::Container { .. } => bail!("Can't add lines to a container"),
            LayoutType::Content(ref mut text) => text.add_child(child, index),
        })
    }
}

impl WidgetRef for Char {
    fn render_ref(&self, area:Rect, buf: &mut Buffer) {
        buf.set_style(area, self.style);
        let mut render_char = self.char;
        if render_char == b'\n' as char {
            render_char = b' ' as char;
        }
        buf.get_mut(area.x, area.y).set_symbol(&render_char.to_string());
    }
}

impl WidgetRef for Span {
    fn render_ref(&self,area:Rect,buf: &mut Buffer) {
        // height is already 1
        if self.characters.len() == 0 {
            let area = Rect { width: 1, ..area };
            buf.set_style(area, self.style);
            return;
        }
        buf.set_style(area, self.style);
        let mut i: u16 = 0;
        for ch in self.characters.iter() {
            let area = Rect {
                x: area.x + i,
                width: 1,
                ..area
            };
            ch.read().unwrap().render_ref(area, buf);
            i += 1;
        }
    }
}

impl WidgetRef for Line {
    fn render_ref(&self,area:Rect,buf: &mut Buffer) {
        // height is already 1
        let len = self.spans.len();
        if len == 0 {
            let area = Rect { width: 1, ..area };
            buf.set_style(area, self.style);
            return;
        }
        buf.set_style(area, self.style);
        let mut offset: u16 = 0;
        for span in self.spans.iter() {
            let span = span.read().unwrap();
            let area = Rect {
                x: area.x + offset,
                y: area.y,
                width: span.characters.len() as u16,
                height: 1,
            };
            span.render_ref(area, buf);
            offset += span.characters.iter().count() as u16;
        }
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
                width: line.read().unwrap().char_len(),
                height: 1,
            };
            line.read().unwrap().render_ref(area, buf);
            line_number += 1;
        }
    }
}

impl WidgetRef for Layout {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        buf.set_style(area, self.style);
        match self.layout {
            LayoutType::Content(ref content) => content.render_ref(area, buf),
            LayoutType::Container { ref split_direction, ref layouts } => {
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
                            layout.read().unwrap().render_ref(area, buf)
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
                            layout.read().unwrap().render_ref(area, buf)
                        }
                    },
                }
            },
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

impl Mother<Char> for Span {
    fn add_child(&mut self, child: Char, index: usize) -> ARW<Char> {
        let len = self.characters.len();
        let mut chars: Vec<ARW<Char>> =
            self.characters
                .drain(min(index, len)..len)
                .collect();

        let child = Arc::new(RwLock::new(child));
        self.characters.push(child.clone());
        self.characters.append(&mut chars);

        child
    }
}

impl Mother<Span> for Line {
    fn add_child(&mut self, child: Span, index: usize) -> ARW<Span> {
        let len = self.spans.len();
        let mut spans: Vec<ARW<Span>> =
            self.spans
                .drain(min(index, len)..len)
                .collect();

        let child = Arc::new(RwLock::new(child));
        self.spans.push(child.clone());
        self.spans.append(&mut spans);

        self.spans = spans;
        child
    }
}

impl Mother<Line> for Text {
    fn add_child(&mut self, child: Line, index: usize) -> ARW<Line> {
        let len = self.lines.len();
        let mut lines: Vec<ARW<Line>> =
            self.lines
                .drain(min(index, len)..len)
                .collect();

        let child = Arc::new(RwLock::new(child));
        self.lines.push(child.clone());
        self.lines.append(&mut lines);

        self.lines = lines;
        child
    }
}

impl TryMother<Layout> for Layout {
    fn try_add_child(&mut self, child: Layout, index: usize) -> Result<ARW<Layout>> {
        Ok(match self.layout {
            LayoutType::Content(_) => bail!("Cant add layout to content"),
            LayoutType::Container { ref mut layouts, .. } => {
                let len = layouts.len();
                let mut tail: Vec<ARW<Layout>> =
                    layouts
                        .drain(min(index, len)..len)
                        .collect();

                let child = Arc::new(RwLock::new(child));
                layouts.push(child.clone());
                layouts.append(&mut tail);

                child
            }
        })
    }
}

