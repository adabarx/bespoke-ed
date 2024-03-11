use std::{cell::{Ref, RefCell}, cmp::min, fs, path::PathBuf, rc::Rc, str::Chars, u16};

use anyhow::{bail, Result};
use ratatui::{buffer::Buffer, layout::{Alignment, Rect}, style::Style, widgets::{Widget, WidgetRef}};

use crate::RC;

pub trait Mother<T> {
    fn add_child(&mut self, child: T, index: usize) -> RC<T>;
}

pub trait TryMother<T> {
    fn try_add_child(&mut self, child: T, index: usize) -> Result<RC<T>>;
}

//
// char
//
#[derive(Default, Clone)]
pub struct Char {
    pub char: char,
    pub style: Style,
}

impl WidgetRef for Char {
    fn render_ref(&self, area:Rect, buf: &mut Buffer) {
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
            content: content
                .chars()
                .map(|ch| Rc::new(RefCell::new(
                    Char { char: ch, style: Style::default() }
                )))
                .collect(),
            style: Style::default(),
        }
    }
}

impl Mother<Char> for Span {
    fn add_child(&mut self, child: Char, index: usize) -> RC<Char> {
        let len = self.content.len();
        let mut chars: Vec<RC<Char>> =
            self.content
                .drain(min(index, len)..len)
                .collect();

        let child = Rc::new(RefCell::new(child));
        self.content.push(child.clone());
        self.content.append(&mut chars);

        child
    }
}

impl Mother<Span> for Line {
    fn add_child(&mut self, child: Span, index: usize) -> RC<Span> {
        let len = self.spans.len();
        let mut spans: Vec<RC<Span>> =
            self.spans
                .drain(min(index, len)..len)
                .collect();

        let child = Rc::new(RefCell::new(child));
        self.spans.push(child.clone());
        self.spans.append(&mut spans);

        self.spans = spans;
        child
    }
}

impl Mother<Line> for Text {
    fn add_child(&mut self, child: Line, index: usize) -> RC<Line> {
        let len = self.lines.len();
        let mut lines: Vec<RC<Line>> =
            self.lines
                .drain(min(index, len)..len)
                .collect();

        let child = Rc::new(RefCell::new(child));
        self.lines.push(child.clone());
        self.lines.append(&mut lines);

        self.lines = lines;
        child
    }
}

impl TryMother<Layout> for Layout {
    fn try_add_child(&mut self, child: Layout, index: usize) -> Result<RC<Layout>> {
        Ok(match self.layout {
            LayoutType::Content(_) => bail!("Cant add layout to content"),
            LayoutType::Container { mut layouts, .. } => {
                let len = layouts.len();
                let mut tail: Vec<RC<Layout>> =
                    layouts
                        .drain(min(index, len)..len)
                        .collect();

                let child = Rc::new(RefCell::new(child));
                layouts.push(child.clone());
                layouts.append(&mut tail);

                child
            }
        })
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
    pub fn raw<T: Into<String>>(input: T) -> Line {
        let spans: String = input.into();
        Line {
            spans: spans
                .split_inclusive(' ')
                .map(|sp| Rc::new(RefCell::new(Span::raw(sp))))
                .collect(),
            ..Default::default()
        }
    }
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
    pub fn raw(input: String) -> Text {
        Text {
            lines: input
                .split('\n')
                .map(|ln| Rc::new(RefCell::new(Line::raw(ln))))
                .collect(),
            ..Default::default()
        }
    }

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
pub struct Layout {
    pub style: Style,
    pub layout: LayoutType,
}

#[derive(Clone)]
pub enum LayoutType {
    Container {
        split_direction: SplitDirection,
        layouts: Vec<RC<Layout>>,
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

impl TryMother<Line> for Layout {
    fn try_add_child(&mut self, child: Line, index: usize) -> Result<RC<Line>> {
        Ok(match self.layout {
            LayoutType::Container { .. } => bail!("Can't add lines to a container"),
            LayoutType::Content(mut text) => text.add_child(child, index),
        })
    }
}

impl<'a> WidgetRef for Layout {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        buf.set_style(area, self.style);
        match self.layout {
            LayoutType::Content(content) => content.render_ref(area, buf),
            LayoutType::Container { split_direction, layouts } => {
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

