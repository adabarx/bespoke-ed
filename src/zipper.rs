//time 2 rewrite
use std::cmp::min;

use async_trait::async_trait;
use either::*;
use tokio::sync::RwLock;

use crate::{primatives::{Char, Line, Root, Span, Text, Window}, ARW};

#[async_trait]
pub trait Zipper {
    // async fn insert(&mut self, char: char) -> Box<dyn Zipper + Send>;
    // async fn delete(&mut self);

    async fn parent(self) -> Box<dyn Zipper + Send>;
    async fn child(self, index: usize) -> Box<dyn Zipper + Send>;

    async fn move_left(self) -> Box<dyn Zipper + Send>;
    async fn move_right(self) -> Box<dyn Zipper + Send>;
}

#[derive(Clone)]
pub struct RootZipper {
    focus: &'static RwLock<Root>,
    children: Vec<ARW<Window>>
}

#[derive(Clone)]
pub struct WindowZipper {
    focus: ARW<Window>,
    parent: Box<Either<RootZipper, WindowZipper>>,
    left: Vec<Either<ARW<Window>, ARW<Text>>>,
    right: Vec<Either<ARW<Window>, ARW<Text>>>,
    children: Vec<Either<ARW<Window>, ARW<Text>>>,
}

#[derive(Clone)]
pub struct TextZipper {
    focus: ARW<Text>,
    parent: WindowZipper,
    left: Vec<Either<ARW<Window>, ARW<Text>>>,
    right: Vec<Either<ARW<Window>, ARW<Text>>>,
    children: Vec<ARW<Line>>
}

#[derive(Clone)]
pub struct LineZipper {
    focus: ARW<Line>,
    parent: TextZipper,
    left: Vec<ARW<Line>>,
    right: Vec<ARW<Line>>,
    children: Vec<ARW<Span>>
}

#[derive(Clone)]
pub struct SpanZipper {
    focus: ARW<Span>,
    parent: LineZipper,
    left: Vec<ARW<Span>>,
    right: Vec<ARW<Span>>,
    children: Vec<ARW<Char>>
}

#[derive(Clone)]
pub struct CharZipper {
    focus: ARW<Char>,
    parent: SpanZipper,
    left: Vec<ARW<Char>>,
    right: Vec<ARW<Char>>,
}


impl RootZipper {
    pub async fn new(root: &'static RwLock<Root>) -> Self {
        Self {
            focus: root,
            children: root.read().await.children.clone(),
        }
    }
}

impl WindowZipper {
    pub async fn new(index: usize, parent: Either<RootZipper, WindowZipper>) -> Self {
        let siblings: Vec<_> = match parent {
            Left(ref rz) => rz.children.iter().cloned().map(|c| Left(c)).collect(),
            Right(ref wz) => wz.children.clone(),
        };
        let index = min(index, siblings.len());
        let focus = siblings[index].clone().left().unwrap();
        let children = focus.read().await.children.clone();

        Self {
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
        let siblings = parent.children.clone();
        let index = min(index, siblings.len());
        let focus = siblings[index].clone().right().unwrap();
        let children = focus.read().await.lines.clone();

        Self {
            focus,
            children,
            parent,
            left: siblings[0..index].iter().cloned().collect(),
            right: siblings[index + 1..].iter().cloned().collect(),
        }
    }
}

impl LineZipper {
    pub async fn new(index: usize, parent: TextZipper) -> Self {
        let siblings = parent.children.clone();
        let index = min(index, siblings.len());
        let focus = siblings[index].clone();
        let children = focus.read().await.spans.clone();

        Self {
            focus,
            children,
            parent,
            left: siblings[0..index].iter().cloned().collect(),
            right: siblings[index + 1..].iter().cloned().collect(),
        }
    }
}

impl SpanZipper {
    pub async fn new(index: usize, parent: LineZipper) -> Self {
        let siblings = parent.children.clone();
        let index = min(index, siblings.len());
        let focus = siblings[index].clone();
        let children = focus.read().await.characters.clone();

        Self {
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
            focus,
            parent,
            left: siblings[0..index].iter().cloned().collect(),
            right: siblings[index + 1..].iter().cloned().collect(),
        }
    }
}


#[async_trait]
impl Zipper for RootZipper {
    async fn parent(self) -> Box<dyn Zipper + Send> {
        Box::new(self)
    }
    async fn child(self, index: usize) -> Box<dyn Zipper + Send> {
        let the_kids = self.children.clone();
        let index = min(index, the_kids.len());
        Box::new(WindowZipper::new(index, Left(self)).await)
    }

    async fn move_left(self) -> Box<dyn Zipper + Send> {
        Box::new(self)
    }
    async fn move_right(self) -> Box<dyn Zipper + Send> {
        Box::new(self)
    }
}

#[async_trait]
impl Zipper for WindowZipper {
    async fn parent(self) -> Box<dyn Zipper + Send> {
        match *self.parent {
            Left(rz) => Box::new(rz) as Box<dyn Zipper + Send>,
            Right(wz) => Box::new(wz) as Box<dyn Zipper + Send>,
        }
    }
    async fn child(self, index: usize) -> Box<dyn Zipper + Send> {
        let the_kids = self.children.clone();
        let len = the_kids.len();
        if len == 0 { return Box::new(self) }
        let index = min(index, len);
        match the_kids[index] {
            Left(_) => Box::new(WindowZipper::new(index, Right(self)).await),
            Right(_) => Box::new(TextZipper::new(index, self).await),
        }
    }

    async fn move_left(self) -> Box<dyn Zipper + Send> {
        let index = self.left.len();
        if index == 0 { return Box::new(self) }
        let parent = *self.parent;

        for_both!(parent, p => p.child(index - 1).await)
    }
    async fn move_right(self) -> Box<dyn Zipper + Send> {
        let index = self.left.len();
        if self.right.len() == 0 { return Box::new(self) }
        let parent = *self.parent;

        for_both!(parent, p => p.child(index + 1).await)
    }
}

#[async_trait]
impl Zipper for TextZipper {
    async fn parent(self) -> Box<dyn Zipper + Send> {
        Box::new(self.parent)
    }
    async fn child(self, index: usize) -> Box<dyn Zipper + Send> {
        let the_kids = self.children.clone();
        let index = min(index, the_kids.len());
        Box::new(LineZipper::new(index, self).await)
    }

    async fn move_left(self) -> Box<dyn Zipper + Send> {
        let index = self.left.len();
        if index == 0 { return Box::new(self) }

        self.parent.child(index - 1).await
    }
    async fn move_right(self) -> Box<dyn Zipper + Send> {
        let index = self.left.len();
        if self.right.len() == 0 { return Box::new(self) }

        self.parent.child(index + 1).await
    }
}

#[async_trait]
impl Zipper for LineZipper {
    async fn parent(self) -> Box<dyn Zipper + Send> {
        Box::new(self.parent)
    }
    async fn child(self, index: usize) -> Box<dyn Zipper + Send> {
        let the_kids = self.children.clone();
        let index = min(index, the_kids.len());
        Box::new(SpanZipper::new(index, self).await)
    }

    async fn move_left(self) -> Box<dyn Zipper + Send> {
        let index = self.left.len();
        if index == 0 { return Box::new(self) }

        self.parent.child(index - 1).await
    }
    async fn move_right(self) -> Box<dyn Zipper + Send> {
        let index = self.left.len();
        if self.right.len() == 0 { return Box::new(self) }

        self.parent.child(index + 1).await
    }
}

#[async_trait]
impl Zipper for SpanZipper {
    async fn parent(self) -> Box<dyn Zipper + Send> {
        Box::new(self.parent)
    }
    async fn child(self, index: usize) -> Box<dyn Zipper + Send> {
        let the_kids = self.children.clone();
        let index = min(index, the_kids.len());
        Box::new(CharZipper::new(index, self).await)
    }

    async fn move_left(self) -> Box<dyn Zipper + Send> {
        let index = self.left.len();
        if index == 0 { return Box::new(self) }

        self.parent.child(index - 1).await
    }
    async fn move_right(self) -> Box<dyn Zipper + Send> {
        let index = self.left.len();
        if self.right.len() == 0 { return Box::new(self) }

        self.parent.child(index + 1).await
    }
}
#[async_trait]
impl Zipper for CharZipper {
    async fn parent(self) -> Box<dyn Zipper + Send> {
        Box::new(self.parent)
    }
    async fn child(self, index: usize) -> Box<dyn Zipper + Send> {
        let _ = index;
        Box::new(self)
    }

    async fn move_left(self) -> Box<dyn Zipper + Send> {
        let index = self.left.len();
        if index == 0 { return Box::new(self) }

        self.parent.child(index - 1).await
    }
    async fn move_right(self) -> Box<dyn Zipper + Send> {
        let index = self.left.len();
        if self.right.len() == 0 { return Box::new(self) }

        self.parent.child(index + 1).await
    }
}
