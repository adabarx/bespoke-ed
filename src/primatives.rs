use std::{cmp::min, sync::Arc, u16};

use async_trait::async_trait;
use tokio::{sync::RwLock, task::JoinSet};
use anyhow::{bail, Result};
use ratatui::{buffer::Buffer, layout::{Alignment, Rect}, style::Style, widgets::WidgetRef};

use crate::ARW;

pub trait Mother<T> {
    fn add_child(&mut self, child: T, index: usize) -> ARW<T>;
}

pub trait TryMother<T> {
    fn try_add_child(&mut self, child: T, index: usize) -> Result<ARW<T>>;
}

#[async_trait]
pub trait AsyncWidget<T: WidgetRef> {
    async fn async_render(&self) -> T;
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

impl TryMother<Line> for Layout {
    fn try_add_child(&mut self, child: Line, index: usize) -> Result<ARW<Line>> {
        Ok(match self.layout {
            LayoutType::Container { .. } => bail!("Can't add lines to a container"),
            LayoutType::Content(ref mut text) => text.add_child(child, index),
        })
    }
}

#[async_trait]
impl AsyncWidget<SpanRender> for Span {
    async fn async_render(&self) -> SpanRender {
        let mut set = JoinSet::new();
        for char in self.characters.iter().cloned() {
            set.spawn(async move { char.read().await.clone() });
        }
        let mut characters = Vec::new();
        while let Some(Ok(char)) = set.join_next().await {
            characters.push(char);
        }

        SpanRender { characters, ..Default::default() }
    }
}

#[async_trait]
impl AsyncWidget<LineRender> for Line {
    async fn async_render(&self) -> LineRender {
        let mut set = JoinSet::new();
        for span in self.spans.iter().cloned() {
            set.spawn(async move { span.read().await.async_render().await });
        }
        let mut spans = Vec::new();
        while let Some(Ok(span)) = set.join_next().await {
            spans.push(span);
        }

        LineRender { spans, ..Default::default() }
    }
}

#[async_trait]
impl AsyncWidget<TextRender> for Text {
    async fn async_render(&self) -> TextRender {
        let mut set = JoinSet::new();
        for line in self.lines.iter().cloned() {
            set.spawn(async move { line.read().await.async_render().await });
        }
        let mut lines = Vec::new();
        while let Some(Ok(line)) = set.join_next().await {
            lines.push(line);
        }

        TextRender { lines, ..Default::default() }
    }
}

#[async_trait]
impl AsyncWidget<LayoutRender> for Layout {
    async fn async_render(&self) -> LayoutRender {
        LayoutRender {
            style: Style::default(),
            layout: match &self.layout {
                LayoutType::Content(content) => LayoutTypeRender::Content(content.async_render().await),
                LayoutType::Container { split_direction, layouts } =>{
                    let mut set = JoinSet::new();
                    for layout in layouts.iter().cloned() {
                        set.spawn(async move { layout.read().await.async_render().await });
                    }
                    let mut layouts = Vec::new();
                    while let Some(Ok(render)) = set.join_next().await {
                        layouts.push(render)
                    }

                    LayoutTypeRender::Container {
                        split_direction: split_direction.clone(),
                        layouts 
                    }  
                },
            }
        }
    }
}

pub enum LayoutTypeRender {
    Container {
        split_direction: SplitDirection,
        layouts: Vec<LayoutRender>,
    },
    Content(TextRender),
}

#[derive(Default)]
pub struct SpanRender {
    pub characters: Vec<Char>,
    pub style: Style,
    pub alignment: Option<Alignment>,
}

#[derive(Default)]
pub struct LineRender {
    pub spans: Vec<SpanRender>,
    pub style: Style,
    pub alignment: Option<Alignment>,
}

#[derive(Default)]
pub struct TextRender {
    pub lines: Vec<LineRender>,
    pub style: Style,
    pub alignment: Option<Alignment>,
}

pub struct LayoutRender {
    style: Style,
    layout: LayoutTypeRender,
}

impl WidgetRef for Char {
    fn render_ref(&self,area:Rect,buf: &mut Buffer) {
        buf.set_style(area, self.style);
        buf.get_mut(area.x, area.y).set_symbol(&self.char.to_string());
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

impl WidgetRef for LayoutRender {
    fn render_ref(&self,area:Rect,buf: &mut Buffer) {
        buf.set_style(area, self.style);
        match self.layout {
            LayoutTypeRender::Content(ref content) => content.render_ref(area, buf),
            LayoutTypeRender::Container { ref split_direction, ref layouts } => {
                let windows: u16 = layouts.len().try_into().unwrap();
                if windows == 0 { return (); }
                match split_direction {
                    SplitDirection::Horizontal => {
                        // split is horizontal. nested containers are stacked vertically
                        let offset = area.height / windows;
                        for (i, layout) in layouts.iter().enumerate() {
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
                        for (i, layout) in layouts.iter().enumerate() {
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
                width: line.spans
                    .iter()
                    .fold(0_u16, |acc, sp| acc + sp.characters.len() as u16),
                height: 1,
            };
            line.render_ref(area, buf);
            line_number += 1;
        }
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

