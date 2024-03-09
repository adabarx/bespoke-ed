use std::{cell::RefCell, rc::Rc, str::Chars, u16};

use ratatui::{buffer::Buffer, layout::{Alignment, Rect}, style::Style, widgets::{Widget, WidgetRef}};

type RC<T> = Rc<RefCell<T>>;

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

impl WidgetRef for Span {
    fn render_ref(&self,area:Rect,buf: &mut Buffer) {
        buf.set_style(area, self.style);
        let mut i: u16 = 0;
        for ch in self.content.iter() {
            let area = Rect {
                x: area.x + i,
                y: area.y,
                width: 1,
                height: 1,
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
    pub fn char_len(&self) -> u16 {
        self.spans
            .iter()
            .map(|sp| sp.borrow().content.len() as u16)
            .sum()
    }
}

impl WidgetRef for Line {
    fn render_ref(&self,area:Rect,buf: &mut Buffer) {
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


