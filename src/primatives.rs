use std::{cell::RefCell, rc::Rc, u16};

use ratatui::{buffer::Buffer, layout::{Alignment, Rect}, style::Style, widgets::{Widget, WidgetRef}};

type RC<T> = Rc<RefCell<T>>;

//
// span
//

#[derive(Clone)]
pub struct Span {
    pub content: String,
    pub style: Style,
}
 impl Span { 
    pub fn raw<T: Into<String>>(content: T) -> Span {
        Span {
            content: content.into(),
            style: Style::default(),
        }
    }
}

impl Default for Span {
    fn default() -> Self {
        Self {
            content: "".into(),
            style: Style::default(),
        }
    }
}

impl WidgetRef for Span {
    fn render_ref(&self,area:Rect,buf: &mut Buffer) {
        let mut i: u16 = 0;
        for ch in self.content.chars() {
            buf.get_mut(area.x + i, area.y).set_symbol(&ch.to_string());
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

impl WidgetRef for Line {
    fn render_ref(&self,area:Rect,buf: &mut Buffer) {
        let mut offset: u16 = 0;
        for span in self.spans.iter() {
            let span = span.borrow();
            span.render_ref(Rect { x: area.x + offset, ..area }, buf);
            offset += span.content.chars().count() as u16;
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

impl Widget for Text {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.render_ref(area, buf);
    }
}

impl WidgetRef for Text {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        buf.set_style(area, self.style);
        let mut line_number: u16 = 0;
        for line in self.lines.iter() {
            line.borrow().render_ref(Rect { y: area.y + line_number, ..area }, buf);
            line_number += 1;
        }
    }
}


