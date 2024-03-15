//time 2 rewrite
use std::{cmp::min, collections::VecDeque};

use ratatui::style::{Color, Style};
use anyhow::{anyhow, Result};
use async_trait::async_trait;

use crate::{primatives::{Char, Layout, LayoutType, Line, Span, TryMother, Mother}, ARW};

#[async_trait]
trait Zipper {
    async fn prev(self) -> Option<LayoutZipper>;

    async fn mother(self) -> MoveResult;
    async fn try_add_child(&mut self, child: Node, index: usize) -> Result<()>;
    async fn daughter(self, index: usize) -> MoveResult;

    async fn left_sister(self) -> MoveResult;
    async fn right_sister(self) -> MoveResult;

    async fn left_aunt(self) -> MoveResult;
    async fn right_aunt(self) -> MoveResult;

    async fn left_cousin(self, index: usize) -> MoveResult;
    async fn right_cousin(self, index: usize) -> MoveResult;

    async fn left_sister_or_cousin(self) -> MoveResult;
    async fn right_sister_or_cousin(self) -> MoveResult;

    async fn replace_focus(self, new_node: Node) -> LayoutZipper;
}

#[derive(Clone)]
pub enum MoveResult {
    Moved(LayoutZipper),
    DidntMove(LayoutZipper)
}

impl MoveResult {
    pub fn unwrap(self) -> LayoutZipper {
        match self {
            MoveResult::Moved(zip) => zip,
            MoveResult::DidntMove(zip) => zip,
        }
    }

    pub fn inner_mut(&mut self) -> &mut LayoutZipper {
        match self {
            MoveResult::Moved(zip) => zip,
            MoveResult::DidntMove(zip) => zip,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
enum PrevDir {
    Parent,
    Left,
    Right,
}

#[derive(Clone)]
struct Breadcrumb {
    zipper: Box<LayoutZipper>,
    direction: PrevDir,
}

enum NodeResult {
    Success(Node),
    Failed(Node),
}

#[derive(Clone)]
pub enum Node {
    Layout(ARW<Layout>),
    Line(ARW<Line>),
    Span(ARW<Span>),
    Char(ARW<Char>),
}

impl Node {
    pub fn get_layout(&self) -> Option<ARW<Layout>> {
        if let Node::Layout(layout) = self {
            Some(layout.clone())
        } else {
            None
        }
    }

    async fn try_add_child(&mut self, child: Node, index: usize) -> Result<Node> {
        use Node::*;
        match (self, child) {
            (Layout(mom), Layout(child)) => Ok(Node::Layout(
                (*mom.write().await).try_add_child(child.read().await.clone(), index)?
            )),
            (Layout(mom), Line(child)) => Ok(Node::Line(
                (*mom.write().await).try_add_child(child.read().await.clone(), index)?
            )),
            (Line(mom), Span(child)) => Ok(Node::Span(
                (*mom.write().await).add_child(child.read().await.clone(), index)
            )),
            (Span(mom), Char(child)) => Ok(Node::Char(
                (*mom.write().await).add_child(child.read().await.clone(), index)
            )),
            _ => Err(anyhow!("this child does not please mother")),
        }
    }

    pub async fn get_children(&self) -> Option<Vec<Node>> {
        // returns None if node doesn't carry children
        // returns an empty vec if the node can carry
        // children but currently doesn't
        match self {
            Node::Layout(layout) => {
                let layout = layout.read().await.layout.clone();
                Some(match layout {
                    LayoutType::Content(text) => text.lines
                        .iter()
                        .map(|l| Node::Line(l.clone()))
                        .collect(),
                    LayoutType::Container { layouts, .. } => layouts
                        .iter()
                        .map(|l| Node::Layout(l.clone()))
                        .collect(),
                })
            },
            Node::Span(span) => Some(
                span.read().await.characters
                    .iter()
                    .map(|ch| Node::Char(ch.clone()))
                    .collect()
            ),
            Node::Line(line) => Some(
                line.read().await.spans
                    .iter()
                    .map(|sp| Node::Span(sp.clone()))
                    .collect()
            ),
            Node::Char(_) => None,
        }
    }

    pub async fn highlight(&mut self) {
        match self {
            Node::Line(line) => {
                line.write().await.style.bg = Some(Color::White);
                line.write().await.style.fg = Some(Color::Black);
            },
            Node::Span(span) => {
                span.write().await.style.bg = Some(Color::White);
                span.write().await.style.fg = Some(Color::Black);
            },
            Node::Char(ch) => {
                ch.write().await.style.bg = Some(Color::White);
                ch.write().await.style.fg = Some(Color::Black);
            },
            Node::Layout(layout) => {
                layout.write().await.style.bg = Some(Color::White);
                layout.write().await.style.fg = Some(Color::Black);
            },
        }
    }

    pub async fn no_highlight(&mut self) {
        match self {
            Node::Line(line) => line.write().await.style = Style::default(),
            Node::Span(span) => span.write().await.style = Style::default(),
            Node::Char(char) => char.write().await.style = Style::default(),
            Node::Layout(layout) => layout.write().await.style = Style::default(),
        }
    }
}

pub struct LayoutCrumb {

}

#[derive(Clone)]
pub struct LayoutZipper {
    previous: Option<Breadcrumb>,
    focus: Node,
    children: Vec<Node>,
    left: Vec<Node>,
    right: VecDeque<Node>,
}

impl LayoutZipper {
    pub async fn new(root: Node) -> Self {
        let children = root.get_children().await.unwrap();
        Self {
            focus: root,
            children,
            previous: None,
            left: Vec::new(),
            right: VecDeque::new(),
        }
    }

    pub async fn try_add_child(&mut self, child: Node, index: usize) -> Result<()> {
        self.focus.try_add_child(child.clone(), index).await.unwrap();

        let len = self.children.len();
        let mut children: Vec<Node> = self.children.drain(min(index, len)..len).collect();
        self.children.push(child);
        self.children.append(&mut children);
        Ok(())
    }

    pub async fn move_to_child(mut self, index: usize) -> MoveResult {
        self.focus.no_highlight();
        let mut result = self.daughter(index).await;
        result.inner_mut().focus.highlight();
        result
    }

    pub async fn move_to_prev(mut self) -> Option<LayoutZipper> {
        self.focus.no_highlight();
        let mut rv = self.prev().await.unwrap();
        rv.focus.highlight();
        Some(rv)
    }

    pub async fn try_move_right(mut self) -> MoveResult {
        self.focus.no_highlight();
        let mut result = self.right_sister().await;
        result.inner_mut().focus.highlight();
        result
    }

    pub async fn try_move_left(mut self) -> MoveResult {
        self.focus.no_highlight();
        let mut result = self.left_sister().await;
        result.inner_mut().focus.highlight();
        result
    }

    pub async fn move_left_catch_ignore(self) -> LayoutZipper {
        self.try_move_right().await.unwrap()
    }

    pub async fn move_right_catch_ignore(self) -> LayoutZipper {
        self.try_move_right().await.unwrap()
    }

    pub async fn go_back_to_parent(mut self) -> MoveResult {
        self.focus.no_highlight();
        let mut result = self.mother().await;
        result.inner_mut().focus.highlight();
        result
    }

    pub async fn move_right_or_cousin(mut self) -> MoveResult {
        self.focus.no_highlight();
        let mut result = self.right_sister_or_cousin().await;
        result.inner_mut().focus.highlight();
        result
    }

    pub async fn move_left_or_cousin(mut self) -> MoveResult {
        self.focus.no_highlight();
        let mut result = self.left_sister_or_cousin().await;
        result.inner_mut().focus.highlight();
        result
    }

    pub async fn add_child(mut self, node: Node, index: usize) -> LayoutZipper {
        let len = self.children.len();
        if index >= len {
            self.children.push(node);
            return self;
        }

        let mut children = self.children[0..index].to_vec();
        let mut child = vec![node];
        let mut the_rest = self.children[index + 1..len].to_vec();
        children.append(&mut child);
        children.append(&mut the_rest);

        self.children = children;
        self
    }

    pub async fn replace_focus(mut self, new_node: Node) -> LayoutZipper {
        self.children = new_node.get_children().await.unwrap_or(Vec::new());
        self.focus = new_node;
        self
    }
}

#[async_trait]
impl Zipper for MoveResult {
    async fn mother(self) -> MoveResult {
        if let MoveResult::DidntMove(_) = self { return self }
        self.unwrap().mother().await
    }

    async fn try_add_child(&mut self, child: Node, index: usize) -> Result<()> {
        self.inner_mut().try_add_child(child, index).await
    }

    async fn replace_focus(self, new_node: Node) -> LayoutZipper {
        if let MoveResult::DidntMove(_) = self { return self.unwrap() }
        self.unwrap().replace_focus(new_node).await
    }    

    async fn right_aunt(self) -> MoveResult {
        if let MoveResult::DidntMove(_) = self { return self }
        self.unwrap().right_aunt().await
    }

    async fn left_aunt(self) -> MoveResult {
        if let MoveResult::DidntMove(_) = self { return self }
        self.unwrap().left_aunt().await
    }

    async fn right_cousin(self, index: usize) -> MoveResult {
        if let MoveResult::DidntMove(_) = self { return self }
        self.unwrap().right_cousin(index).await
    }

    async fn left_cousin(self, index: usize) -> MoveResult {
        if let MoveResult::DidntMove(_) = self { return self }
        self.unwrap().left_cousin(index).await
    }

    async fn daughter(self, index: usize) -> MoveResult {
        if let MoveResult::DidntMove(_) = self { return self }
        self.unwrap().daughter(index).await
    }

    async fn left_sister(self) -> MoveResult {
        if let MoveResult::DidntMove(_) = self { return self }
        self.unwrap().left_sister().await
    }

    async fn right_sister(self) -> MoveResult {
        if let MoveResult::DidntMove(_) = self { return self }
        self.unwrap().right_sister().await
    }

    async fn left_sister_or_cousin(self) -> MoveResult {
        if let MoveResult::DidntMove(_) = self { return self }
        self.unwrap().left_sister_or_cousin().await
    }

    async fn right_sister_or_cousin(self) -> MoveResult {
        if let MoveResult::DidntMove(_) = self { return self }
        self.unwrap().right_sister_or_cousin().await
    }

    async fn prev(self) -> Option<LayoutZipper> {
        if let MoveResult::DidntMove(_) = self { return None }
        self.unwrap().prev().await
    }

}

#[async_trait]
impl Zipper for LayoutZipper {
    async fn mother(self) -> MoveResult {
        if self.previous.is_none() { return MoveResult::DidntMove(self) }
        let prev = self.previous.unwrap();
        match prev.direction {
            PrevDir::Parent => MoveResult::Moved(*prev.zipper),
            PrevDir::Left => prev.zipper.mother().await,
            PrevDir::Right => prev.zipper.mother().await,
        }
    }

    async fn try_add_child(&mut self, child: Node, index: usize) -> Result<()> {
        let children = &mut self.children;
        let tail = &mut children.drain(index..).collect();
        children.push(child);
        children.append(tail);
        Ok(())
    }

    async fn right_aunt(self) -> MoveResult {
        let og = self.clone();
        let result = self.mother().await;
        if let MoveResult::DidntMove(_) = result {
            return MoveResult::DidntMove(og);
        }
        let result = result.right_sister().await;
        if let MoveResult::DidntMove(_) = result {
            return MoveResult::DidntMove(og);
        }
        result
    }

    async fn left_aunt(self) -> MoveResult {
        let og = self.clone();
        let result = self.mother().await;
        if let MoveResult::DidntMove(_) = result {
            return MoveResult::DidntMove(og);
        }
        let result = result.unwrap().left_sister().await;
        if let MoveResult::DidntMove(_) = result {
            return MoveResult::DidntMove(og);
        }
        result
    }

    async fn right_cousin(self, index: usize) -> MoveResult {
        let og = self.clone();
        let result = self.right_aunt().await;
        if let MoveResult::DidntMove(_) = result {
            return MoveResult::DidntMove(og);
        }
        let result = result.unwrap().daughter(index).await;
        if let MoveResult::DidntMove(_) = result {
            return MoveResult::DidntMove(og);
        }
        result
    }

    async fn left_cousin(self, index: usize) -> MoveResult {
        let og = self.clone();
        let result = self.left_aunt().await;
        if let MoveResult::DidntMove(_) = result {
            return MoveResult::DidntMove(og);
        }
        let result = result.unwrap().daughter(index).await;
        if let MoveResult::DidntMove(_) = result {
            return MoveResult::DidntMove(og);
        }
        result
    }

    async fn daughter(self, mut index: usize) -> MoveResult {
        let len = self.children.len();
        if len == 0 { return MoveResult::DidntMove(self) }
        if index >= len { 
            index = len - 1;
        }

        let left = self.children[0..index]
            .iter()
            .cloned()
            .collect();
        let right = self.children[index + 1..len]
            .iter()
            .cloned()
            .collect();
        let focus = self.children[index].clone();
        let children = focus.get_children().await.unwrap_or(Vec::new());
        let previous = Some(Breadcrumb { zipper: Box::new(self), direction: PrevDir::Parent });
        
        MoveResult::Moved(LayoutZipper { previous, focus, children, left, right })
    }

    async fn left_sister(self) -> MoveResult {
        if let Some(prev) = self.previous.as_ref() {
            if prev.direction == PrevDir::Left {
                return MoveResult::Moved(self.move_to_prev().await.unwrap());
            }
        }

        let mut left = self.left.clone();
        let focus = if let Some(node) = left.pop() { node }
            else { return MoveResult::DidntMove(self); };

        let mut right = self.right.clone();
        right.push_front(self.focus.clone());
        let children = focus.get_children().await.unwrap_or(Vec::new());
        let previous = Some(Breadcrumb { zipper: Box::new(self), direction: PrevDir::Right });

        MoveResult::Moved(LayoutZipper { focus, previous, children, left, right })
    }

    async fn right_sister(self) -> MoveResult {
        if let Some(prev) = self.previous.as_ref() {
            if prev.direction == PrevDir::Right {
                return MoveResult::Moved(self.move_to_prev().await.unwrap());
            }
        }

        let mut right = self.right.clone();
        let focus = if let Some(node) = right.pop_front() { node }
            else { return MoveResult::DidntMove(self); };

        let mut left = self.left.clone();
        left.push(self.focus.clone());
        let children = focus.get_children().await.unwrap_or(Vec::new());
        let previous = Some(Breadcrumb { zipper: Box::new(self), direction: PrevDir::Left });

        MoveResult::Moved(LayoutZipper { focus, previous, children, left, right })
    }

    async fn left_sister_or_cousin(self) -> MoveResult {
        let og = self.clone();
        let sister = self.left_sister().await;
        if let MoveResult::Moved(_) = sister {
            return sister;
        }
        og.left_cousin(usize::MAX).await
    }

    async fn right_sister_or_cousin(self) -> MoveResult {
        let og = self.clone();
        let sister = self.right_sister().await;
        if let MoveResult::Moved(_) = sister {
            return sister;
        }
        og.right_cousin(0).await
    }

    async fn prev(self) -> Option<LayoutZipper> {
        if self.previous.is_none() { return None }
        Some(*self.previous.unwrap().zipper)
    }

    async fn replace_focus(mut self, new_node: Node) -> LayoutZipper {
        self.children = new_node.get_children().await.unwrap_or(Vec::new()).clone();
        self.focus = new_node.clone();
        self
    }
}
