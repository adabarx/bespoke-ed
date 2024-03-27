
//time 2 rewrite
use std::cmp::min;

use async_trait::async_trait;
use either::*;
use ratatui::layout::Rect;
use tokio::sync::RwLock;

use crate::{primatives::{AsyncWidget, Char, Line, Root, Span, Text, Window}, ARW};

pub type DynZipper = Box<dyn Zipper + Send + Sync>;

#[async_trait]
pub trait Zipper {
    // async fn insert(&mut self, char: char) -> DynZipper;
    // async fn delete(&mut self);
    async fn highlight(&self, hl: bool) {
        let _ = hl;
    }

    async fn parent(&self) -> DynZipper;
    async fn child(&self, index: usize) -> DynZipper;

    async fn move_left(&self) -> DynZipper;
    async fn move_right(&self) -> DynZipper;
}

#[derive(Clone)]
pub struct RootZipper {
    area: Rect,
    focus: &'static RwLock<Root>,
    children: Vec<ARW<Window>>
}

#[derive(Clone)]
pub struct WindowZipper {
    area: Rect,
    focus: ARW<Window>,
    parent: Box<Either<RootZipper, WindowZipper>>,
    left: Vec<Either<ARW<Window>, ARW<Text>>>,
    right: Vec<Either<ARW<Window>, ARW<Text>>>,
    children: Vec<Either<ARW<Window>, ARW<Text>>>,
}

#[derive(Clone)]
pub struct TextZipper {
    area: Rect,
    focus: ARW<Text>,
    parent: WindowZipper,
    left: Vec<Either<ARW<Window>, ARW<Text>>>,
    right: Vec<Either<ARW<Window>, ARW<Text>>>,
    children: Vec<ARW<Line>>
}

#[derive(Clone)]
pub struct LineZipper {
    row: usize,
    focus: ARW<Line>,
    parent: TextZipper,
    left: Vec<ARW<Line>>,
    right: Vec<ARW<Line>>,
    children: Vec<ARW<Span>>
}

#[derive(Clone)]
pub struct SpanZipper {
    column: usize, // column of the first character
    focus: ARW<Span>,
    parent: LineZipper,
    left: Vec<ARW<Span>>,
    right: Vec<ARW<Span>>,
    children: Vec<ARW<Char>>
}

#[derive(Clone)]
pub struct CharZipper {
    column: usize,
    focus: ARW<Char>,
    parent: SpanZipper,
    left: Vec<ARW<Char>>,
    right: Vec<ARW<Char>>,
}


impl RootZipper {
    pub async fn new(root: &'static RwLock<Root>) -> Self {
        Self {
            focus: root,
            area: root.read().await.area.clone(),
            children: root.read().await.children.clone(),
        }
    }
}

impl WindowZipper {
    pub async fn new(index: usize, parent: Either<RootZipper, WindowZipper>) -> Self {
        let (siblings, area): (Vec<_>, Rect) = match parent {
            Left(ref rz) => (rz.children.iter().cloned().map(|c| Left(c)).collect(), rz.area.clone()),
            Right(ref wz) => (wz.children.clone(), wz.area.clone()),
        };
        let index = min(index, siblings.len());
        let focus = siblings[index].clone().left().unwrap();
        let children = focus.read().await.children.clone();

        Self {
            area,
            focus,
            children,
            parent: Box::new(parent),
            left: siblings[0..index].iter().cloned().collect(),
            right: siblings[index + 1..].iter().cloned().collect(),
        }
    }
}

impl TextZipper {
    pub async fn new(index: usize, parent: WindowZipper) -> Self {
        let area = parent.area.clone();
        let siblings = parent.children.clone();
        let index = min(index, siblings.len());
        let focus = siblings[index].clone().right().unwrap();
        let children = focus.read().await.lines.clone();

        Self {
            focus,
            children,
            parent,
            area,
            left: siblings[0..index].iter().cloned().collect(),
            right: siblings[index + 1..].iter().cloned().collect(),
        }
    }
}

impl LineZipper {
    pub async fn new(index: usize, parent: TextZipper) -> Self {
        let siblings = parent.children.clone();
        let row = min(index, siblings.len());
        let focus = siblings[row].clone();
        let children = focus.read().await.spans.clone();

        let par_top = parent.focus.read().await.top;
        let pos_relative = index.saturating_sub(par_top);

        if pos_relative < par_top {
            parent.focus.write().await.top -= par_top - pos_relative;
        } else if pos_relative > par_top + parent.area.height as usize {
            parent.focus.write().await.top += pos_relative - (par_top + parent.area.height as usize);
        }

        Self {
            row,
            focus,
            children,
            parent,
            left: siblings[0..row].iter().cloned().collect(),
            right: siblings[row + 1..].iter().cloned().collect(),
        }
    }
}

impl SpanZipper {
    pub async fn new(index: usize, parent: LineZipper) -> Self {
        let siblings = parent.children.clone();

        let mut column = 0_usize;
        for (i, sib) in siblings.iter().enumerate() {
            if i < index {
                column += sib.read().await.characters.len();
            } else {
                break
            }
        }

        let index = min(index, siblings.len() - 1);
        let focus = siblings[index].clone();
        let children = focus.read().await.characters.clone();

        Self {
            column,
            focus,
            children,
            parent,
            left: siblings[0..index].iter().cloned().collect(),
            right: siblings[index + 1..].iter().cloned().collect(),
        }
    }
}

impl CharZipper {
    pub async fn new(index: usize, parent: SpanZipper) -> Self {
        let siblings = parent.children.clone();
        let index = min(index, siblings.len());
        let focus = siblings[index].clone();

        Self {
            column: parent.column + index,
            focus,
            parent,
            left: siblings[0..index].iter().cloned().collect(),
            right: siblings[index + 1..].iter().cloned().collect(),
        }
    }

    pub async fn move_left_or_cousin(&self) -> DynZipper {
        if self.left.len() >  0 {
            self.move_left().await
        } else if self.left.len() == 0 && self.parent.left.len() > 0 {
            let aunt = self.parent.move_left().await;
            aunt.child(usize::MAX).await
        } else {
            Box::new(self.clone())
        }
    }

    pub async fn move_right_or_cousin(&self) -> DynZipper {
        if self.left.len() >  0 {
            self.move_right().await
        } else if self.right.len() == 0 && self.parent.right.len() > 0 {
            let aunt = self.parent.move_right().await;
            aunt.child(0).await
        } else {
            Box::new(self.clone())
        }
    }
}


#[async_trait]
impl Zipper for RootZipper {
    async fn parent(&self) -> DynZipper {
        Box::new(self.clone())
    }
    async fn child(&self, index: usize) -> DynZipper {
        let the_kids = self.children.clone();
        let index = min(index, the_kids.len());
        let child = WindowZipper::new(index, Left(self.clone())).await;
        Box::new(child)
    }

    async fn move_left(&self) -> DynZipper {
        Box::new(self.clone())
    }
    async fn move_right(&self) -> DynZipper {
        Box::new(self.clone())
    }
}

#[async_trait]
impl Zipper for WindowZipper {
    // TODO: highlight/no highlight
    async fn parent(&self) -> DynZipper {
        self.focus.no_highlight().await;
        match *self.parent {
            Left(ref rz) => Box::new(rz.clone()) as DynZipper,
            Right(ref wz) => Box::new(wz.clone()) as DynZipper,
        }
    }
    async fn child(&self, index: usize) -> DynZipper {
        let the_kids = self.children.clone();
        let len = the_kids.len();
        if len == 0 { return Box::new(self.clone()) }
        let index = min(index, len - 1);
        match the_kids[index] {
            Left(_) => Box::new(WindowZipper::new(index, Right(self.clone())).await),
            Right(_) => Box::new(TextZipper::new(index, self.clone()).await),
        }
    }

    async fn move_left(&self) -> DynZipper {
        let index = self.left.len();
        if index == 0 { return Box::new(self.clone()) }
        let parent = *self.parent.clone();

        for_both!(parent, p => p.child(index - 1).await)
    }
    async fn move_right(&self) -> DynZipper {
        let index = self.left.len();
        if self.right.len() == 0 { return Box::new(self.clone()) }
        let parent = *self.parent.clone();

        for_both!(parent, p => p.child(index + 1).await)
    }
}

#[async_trait]
impl Zipper for TextZipper {
    async fn parent(&self) -> DynZipper {
        self.highlight(false).await;
        self.parent.highlight(true).await;
        Box::new(self.parent.clone())
    }
    async fn child(&self, index: usize) -> DynZipper {
        if self.children.len() == 0 { return Box::new(self.clone()) }
        self.highlight(false).await;
        let the_kids = self.children.clone();
        let index = min(index, the_kids.len());
        let child = LineZipper::new(index, self.clone()).await;
        child.highlight(true).await;
        Box::new(child)
    }

    async fn move_left(&self) -> DynZipper {
        let index = self.left.len();
        if index == 0 { return Box::new(self.clone()) }
        self.highlight(false).await;

        let child = self.parent.child(index - 1).await;
        child.highlight(true).await;
        child
    }
    async fn move_right(&self) -> DynZipper {
        let index = self.left.len();
        if self.right.len() == 0 { return Box::new(self.clone()) }
        self.highlight(false).await;

        let child = self.parent.child(index + 1).await;
        child.highlight(true).await;
        child
    }
}

#[async_trait]
impl Zipper for LineZipper {
    async fn parent(&self) -> DynZipper {
        self.highlight(false).await;
        self.parent.highlight(true).await;
        Box::new(self.parent.clone())
    }
    async fn child(&self, index: usize) -> DynZipper {
        if self.children.len() == 0 { return Box::new(self.clone()) }
        self.highlight(false).await;
        let the_kids = self.children.clone();
        let index = min(index, the_kids.len());
        let child = SpanZipper::new(index, self.clone()).await;
        child.highlight(true).await;
        Box::new(child)
    }

    async fn move_left(&self) -> DynZipper {
        let index = self.left.len();
        if index == 0 { return Box::new(self.clone()) }
        self.highlight(false).await;

        let child = self.parent.child(index - 1).await;
        child.highlight(true).await;
        child
    }
    async fn move_right(&self) -> DynZipper {
        let index = self.left.len();
        if self.right.len() == 0 { return Box::new(self.clone()) }
        self.highlight(false).await;

        let child = self.parent.child(index + 1).await;
        child.highlight(true).await;
        child
    }

    async fn highlight(&self, hl: bool) {
        if hl {
            self.focus.highlight().await;
        } else {
            self.focus.no_highlight().await;
        }
    }
}

#[async_trait]
impl Zipper for SpanZipper {
    async fn parent(&self) -> DynZipper {
        self.highlight(false).await;
        self.parent.highlight(true).await;
        Box::new(self.parent.clone())
    }
    async fn child(&self, index: usize) -> DynZipper {
        if self.children.len() == 0 { return Box::new(self.clone()) }
        self.highlight(false).await;
        let the_kids = self.children.clone();
        let index = min(index, the_kids.len());
        let child = CharZipper::new(index, self.clone()).await;
        child.highlight(true).await;
        Box::new(child)
    }

    async fn move_left(&self) -> DynZipper {
        let index = self.left.len();
        if index == 0 { return Box::new(self.clone()) }
        self.highlight(false).await;

        let sib = self.parent.child(index - 1).await;
        sib.highlight(true).await;
        sib
    }
    async fn move_right(&self) -> DynZipper {
        let index = self.left.len();
        if self.right.len() == 0 { return Box::new(self.clone()) }
        self.highlight(false).await;

        let sib = self.parent.child(index + 1).await;
        sib.highlight(true).await;
        sib
    }

    async fn highlight(&self, hl: bool) {
        if hl {
            self.focus.highlight().await;
        } else {
            self.focus.no_highlight().await;
        }
    }
}
#[async_trait]
impl Zipper for CharZipper {
    async fn parent(&self) -> DynZipper {
        self.highlight(false).await;
        self.parent.highlight(true).await;
        Box::new(self.parent.clone())
    }
    async fn child(&self, index: usize) -> DynZipper {
        let _ = index;
        Box::new(self.clone())
    }

    async fn move_left(&self) -> DynZipper {
        let index = self.left.len();
        if index == 0 { return Box::new(self.clone()) }
        self.highlight(false).await;

        let sib = self.parent.child(index - 1).await;
        sib.highlight(true).await;
        sib
    }
    async fn move_right(&self) -> DynZipper {
        let index = self.left.len();
        if self.right.len() == 0 { return Box::new(self.clone()) }
        self.highlight(false).await;

        let sib = self.parent.child(index + 1).await;
        sib.highlight(true).await;
        sib
    }

    async fn highlight(&self, hl: bool) {
        if hl {
            self.focus.highlight().await;
        } else {
            self.focus.no_highlight().await;
        }
    }
}




