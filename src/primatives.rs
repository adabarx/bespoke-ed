use std::{cell::RefCell, rc::Rc};

use ratatui::{buffer::Buffer, layout::{Alignment, Rect}, style::Style, widgets::{Widget, WidgetRef}};

type RC<T> = Rc<RefCell<T>>;

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
            ..Default::default()
        }
    }
}

#[derive(Default)]
pub struct Line {
    pub spans: Vec<RC<Span>>,
    pub style: Style,
    pub alignment: Option<Alignment>,
}

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
        for (line, row) in self.iter().zip(area.rows()) {
            let line_width = line.width() as u16;

            let x_offset = match (self.alignment, line.alignment) {
                (Some(Alignment::Center), None) => area.width.saturating_sub(line_width) / 2,
                (Some(Alignment::Right), None) => area.width.saturating_sub(line_width),
                _ => 0,
            };

            let line_area = Rect {
                x: area.x + x_offset,
                y: row.y,
                width: area.width - x_offset,
                height: 1,
            };

            line.render(line_area, buf);
        }
    }
}


