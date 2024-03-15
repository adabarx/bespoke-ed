use std::{cmp::min, sync::Arc, u16};

use async_trait::async_trait;
use tokio::{sync::RwLock, task::JoinSet};
use anyhow::Result;
use ratatui::{prelude::{Rect, Alignment}, buffer::Buffer, style::Style, widgets::WidgetRef};

use crate::ARW;

pub trait Mother<T> {
    fn add_child(&mut self, child: T, index: usize) -> ARW<T>;
}

pub trait TryMother<T> {
    fn try_add_child(&mut self, child: T, index: usize) -> Result<ARW<T>>;
}

#[async_trait]
pub trait AsyncWidget {
    async fn async_render(&self) -> Box<dyn Render + Send>;
}

#[async_trait]
pub trait ParentWidget<T>
where
    T: AsyncWidget + ?Sized + 'static + Send + Sync
{
    async fn get_children(&self) -> Vec<ARW<T>>;
}

pub trait Render: WidgetRef + Send {
    fn get_width_height(&self) -> (u16, u16);
}

pub trait WindowChild {}

impl WindowChild for Window {}
impl WindowChild for Text {}

impl ParentWidget<Char> for Span {
    async fn get_children(&self) -> Vec<ARW<Char>> {
        self.characters.clone()
    }
}

impl ParentWidget<Span> for Line {
    async fn get_children(&self) -> Vec<ARW<Span>> {
        self.spans.clone()
    }
}

impl ParentWidget<Line> for Text {
    async fn get_children(&self) -> Vec<ARW<Line>> {
        self.lines.clone()
    }
}


#[derive(Default, Clone, PartialEq, Eq)]
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

#[derive(Clone, Copy, Default)]
pub enum SplitDirection {
    #[default]
    Vertical,
    Horizontal,
}


#[derive(Clone, Default)]
pub struct Window {
    pub style: Style,
    pub split_dir: SplitDirection,
    pub windows: Vec<ARW<dyn AsyncWidget>>,
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

    pub async fn is_newline(&self) -> bool {
        if self.characters.len() == 1
            && self.characters[0].read().await.char == b'\n' as char
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

    pub async fn char_len(&self) -> u16 {
        let mut set = JoinSet::new();

        for sp in self.spans.iter().cloned() {
            set.spawn(async move { sp.read().await.characters.len() });
        }

        let mut count = 0;
        while let Some(Ok(len)) = set.join_next().await {
            count += len;
        }
        count as u16
    }
}

#[async_trait]
impl AsyncWidget for Char {
    async fn async_render(&self) -> Box<dyn Render + Send> {
        Box::new(self.clone())
    }
}

#[async_trait]
impl AsyncWidget for Span {
    async fn async_render(&self) -> Box<dyn Render + Send> {
        let mut set = JoinSet::new();
        for (i, char) in self.characters.iter().cloned().enumerate() {
            set.spawn(async move { (i, char.read().await.async_render().await) });
        }
        let mut characters = Vec::new();
        while let Some(Ok(char)) = set.join_next().await {
            characters.push(char);
        }
        characters.sort_by(|a, b| a.0.cmp(&b.0));

        Box::new(SpanRender {
            characters: characters.into_iter().map(|(_, render)| render).collect(),
            style: self.style,
            ..Default::default()
        })
    }
}

#[async_trait]
impl AsyncWidget for Line {
    async fn async_render(&self) -> Box<dyn Render + Send> {
        let mut set = JoinSet::new();
        for (i, span) in self.spans.iter().cloned().enumerate() {
            set.spawn(async move { (i, span.read().await.async_render().await) });
        }
        let mut spans = Vec::new();
        while let Some(Ok(span)) = set.join_next().await {
            spans.push(span);
        }
        spans.sort_by(|a, b| a.0.cmp(&b.0));

        Box::new(LineRender {
            spans: spans.into_iter().map(|(_, render)| render).collect(),
            style: self.style,
            ..Default::default()
        })
    }
}

#[async_trait]
impl AsyncWidget for Text {
    async fn async_render(&self) -> Box<dyn Render + Send> {
        let mut set = JoinSet::new();
        for (i, line) in self.lines.iter().cloned().enumerate() {
            set.spawn(async move { (i, line.read().await.async_render().await) });
        }

        let mut lines = Vec::new();
        while let Some(Ok(line)) = set.join_next().await {
            lines.push(line);
        }
        lines.sort_by(|a, b| a.0.cmp(&b.0));

        Box::new(TextRender { 
            lines: lines.into_iter().map(|r| r.1).collect(),
            style: self.style,
            alignment: self.alignment,
        })
    }
}

type DynRender = Box<dyn Render + Send>;

pub struct WindowRender {
    split_dir: SplitDirection,
    style: Style,
    windows: Vec<Box<dyn WidgetRef + Send>>
}

#[derive(Default)]
pub struct SpanRender {
    pub characters: Vec<DynRender>,
    pub style: Style,
    pub alignment: Option<Alignment>,
}

#[derive(Default)]
pub struct LineRender {
    pub spans: Vec<DynRender>,
    pub style: Style,
    pub alignment: Option<Alignment>,
}

#[derive(Default)]
pub struct TextRender {
    pub lines: Vec<DynRender>,
    pub style: Style,
    pub alignment: Option<Alignment>,
}

impl WidgetRef for Char {
    fn render_ref(&self,area:Rect,buf: &mut Buffer) {
        buf.set_style(area, self.style);
        buf.get_mut(area.x, area.y).set_symbol(&self.char.to_string());
    }
}

impl Render for Char {
    fn get_width_height(&self) -> (u16, u16) {
        (1, 1)
    }
}

impl WidgetRef for SpanRender {
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
            ch.render_ref(area, buf);
            i += 1;
        }
    }
}

impl Render for SpanRender {
    fn get_width_height(&self) -> (u16, u16) {
        (self.characters.len() as u16, 1)
    }
}

impl WidgetRef for LineRender {
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
            let (width, height) = span.get_width_height();
            let area = Rect {
                x: area.x + offset,
                y: area.y,
                width,
                height,
            };
            span.render_ref(area, buf);
            offset += width;
        }
    }
}

impl Render for LineRender {
    fn get_width_height(&self) -> (u16, u16) {
        let width = self.spans
            .iter()
            .fold(0_u16, |acc, width| acc + width.get_width_height().0);
        (width, 1)
    }
}

impl WidgetRef for WindowRender {
    fn render_ref(&self,area:Rect,buf: &mut Buffer) {
        buf.set_style(area, self.style);
        let windows: u16 = self.windows.len() as u16;
        if windows == 0 { return; }
        match self.split_dir {
            SplitDirection::Horizontal => {
                // split is horizontal. nested containers are stacked vertically
                let offset = area.height / windows;
                for (i, layout) in self.windows.iter().enumerate() {
                    let area = Rect::new(
                        area.x,
                        if i == 0 { area.y } else { area.y + offset + 1 },
                        area.width,
                        offset
                    );
                    layout.render_ref(area, buf);
                }
            },
            SplitDirection::Vertical => {
                // split is vertical. nested containers are stacked horizontally
                let offset = area.width / windows;
                for (i, layout) in self.windows.iter().enumerate() {
                    let area = Rect::new(
                        if i == 0 { area.x } else { area.x + offset + 1 },
                        area.y,
                        offset,
                        area.height,
                    );
                    layout.render_ref(area, buf);
                }
            },
        }
    }
}

impl WidgetRef for TextRender {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        buf.set_style(area, self.style);
        let mut line_number: u16 = 0;
        for line in self.lines.iter() {
            let area = Rect {
                x: area.x,
                y: area.y + line_number,
                width: line.get_width_height().0,
                height: 1,
            };
            line.render_ref(area, buf);
            line_number += 1;
        }
    }
}

impl Render for TextRender {
    fn get_width_height(&self) -> (u16, u16) {
        (0, 0)
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

