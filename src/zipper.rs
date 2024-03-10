use std::{cell::RefCell, cmp::min, collections::VecDeque, rc::Rc};

use ratatui::style::{Color, Style};
use anyhow::{anyhow, bail, Result};

use crate::{primatives::{Char, Layout, Line, Mother, Span, TryMother}, RC};

#[derive(Clone)]
pub enum ZipperMoveResult {
    Success(Zipper),
    Failed(Zipper)
}

impl ZipperMoveResult {
    pub fn unwrap(self) -> Zipper {
        match self {
            ZipperMoveResult::Success(zip) => zip,
            ZipperMoveResult::Failed(zip) => zip,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
enum PrevDir {
    Parent { index: usize },
    Left,
    Right,
}

#[derive(Clone)]
struct Breadcrumb {
    zipper: Box<Zipper>,
    direction: PrevDir,
}

enum NodeResult {
    Success(Node),
    Failed(Node),
}

#[derive(Clone)]
pub enum Node {
    Layout(RC<Layout>),
    Line(RC<Line>),
    Span(RC<Span>),
    Char(RC<Char>),
}

impl Node {
    pub fn get_layout(&self) -> Option<RC<Layout>> {
        if let Node::Layout(layout) = self {
            Some(layout.clone())
        } else {
            None
        }
    }

    fn try_add_child(&mut self, child: Node, index: usize) -> Result<Node> {
        use Node::*;
        match (self, child) {
            (Layout(mom), Layout(child)) => Ok(Node::Layout(
                mom.borrow_mut().try_add_child(child.borrow().clone(), index)?
            )),
            (Layout(mom), Line(child)) => Ok(Node::Line(
                mom.borrow_mut().try_add_child(child.borrow().clone(), index)?
            )),
            (Line(mom), Span(child)) => Ok(Node::Span(
                mom.borrow_mut().add_child(child.borrow().clone(), index)
            )),
            (Span(mom), Char(child)) => Ok(Node::Char(
                mom.borrow_mut().add_child(child.borrow().clone(), index)
            )),
            _ => Err(anyhow!("this child does not please mother")),
        }
    }
    pub fn get_children(&self) -> Option<Vec<Node>> {
        // returns None if node doesn't carry children
        // returns an empty vec if the node can carry
        // children but currently doesn't
        match self {
            Node::Layout(layout) => match layout.borrow().clone() { // TODO: get rid of this clone
                Layout::Content(text) => Some(
                    text.lines.iter()
                        .map(|l| Node::Line(l.clone()))
                        .collect()
                ),
                Layout::Container { layouts, .. } => Some(
                    layouts.iter()
                        .map(|l| Node::Layout(l.clone()))
                        .collect()
                ),
            },
            Node::Span(span) => Some(
                span.borrow().content
                    .iter()
                    .map(|ch| Node::Char(ch.clone()))
                    .collect()
            ),
            Node::Line(line) => Some(
                line.borrow().spans
                    .iter()
                    .map(|sp| Node::Span(sp.clone()))
                    .collect()
            ),
            Node::Char(_) => None,
        }
    }

    pub fn highlight(&mut self) {
        match self {
            Node::Line(line) => {
                line.borrow_mut().style.bg = Some(Color::White);
                line.borrow_mut().style.fg = Some(Color::Black);
            },
            Node::Span(span) => {
                span.borrow_mut().style.bg = Some(Color::White);
                span.borrow_mut().style.fg = Some(Color::Black);
            },
            Node::Char(ch) => {
                ch.borrow_mut().style.bg = Some(Color::White);
                ch.borrow_mut().style.fg = Some(Color::Black);
            },
            Node::Layout(_) => (),
        }
    }

    pub fn no_highlight(&mut self) {
        match self {
            Node::Line(line) => line.borrow_mut().style = Style::default(),
            Node::Span(span) => span.borrow_mut().style = Style::default(),
            Node::Char(char) => char.borrow_mut().style = Style::default(),
            Node::Layout(_) => (),
        }
    }
}

#[derive(Clone)]
pub struct Zipper {
    previous: Option<Breadcrumb>,
    focus: Node,
    children: Vec<Node>,
    left: Vec<Node>,
    right: VecDeque<Node>,
}

impl Zipper {
    pub fn new(root: Node) -> Self {
        let children = root.get_children().unwrap();
        Self {
            focus: root,
            children,
            previous: None,
            left: Vec::new(),
            right: VecDeque::new(),
        }
    }

    fn try_add_child(&mut self, child: Node, index: usize) -> Result<()> {
        self.focus.try_add_child(child.clone(), index)?;

        let len = self.children.len();
        let mut children: Vec<Node> = self.children.drain(min(index, len)..len).collect();
        self.children.push(child);
        self.children.append(&mut children);
        Ok(())
    }

    pub fn move_to_child(mut self, index: usize) -> ZipperMoveResult {
        if index >= self.children.len() { return ZipperMoveResult::Failed(self) }
        self.focus.no_highlight();

        let left = self.children[0..index].iter()
            .cloned()
            .collect();
        let right = self.children[index + 1..self.children.len()].iter()
            .cloned()
            .collect();
        let mut focus = self.children[index].clone();
        focus.highlight();

        let children = focus.get_children().unwrap_or(Vec::new());
        let previous = Some(Breadcrumb { zipper: Box::new(self), direction: PrevDir::Parent{ index } });

        ZipperMoveResult::Success(Zipper { focus, previous, children, left, right })
    }

    pub fn move_to_prev(mut self) -> Option<Zipper> {
        if let Some(crumb) = self.previous {
            self.focus.no_highlight();
            let mut rv = *crumb.zipper;
            rv.focus.highlight();
            Some(rv)
        } else {
            None
        }
    }

    pub fn move_left(mut self) -> ZipperMoveResult {
        if let Some(prev) = self.previous.as_ref() {
            if prev.direction == PrevDir::Left {
                return ZipperMoveResult::Success(self.move_to_prev().unwrap());
            }
        }

        let mut left = self.left.clone();
        let mut focus = if let Some(node) = left.pop() { node }
            else { return ZipperMoveResult::Failed(self); };

        self.focus.no_highlight();
        focus.highlight();

        let mut right = self.right.clone();
        right.push_front(self.focus.clone());
        let children = focus.get_children().unwrap_or(Vec::new());
        let previous = Some(Breadcrumb { zipper: Box::new(self), direction: PrevDir::Right });

        ZipperMoveResult::Success(Zipper { focus, previous, children, left, right })
    }

    pub fn move_right(mut self) -> ZipperMoveResult {
        if let Some(prev) = self.previous.as_ref() {
            if prev.direction == PrevDir::Right {
                return ZipperMoveResult::Success(self.move_to_prev().unwrap());
            }
        }

        let mut right = self.right.clone();
        let mut focus = if let Some(node) = right.pop_front() { node }
            else { return ZipperMoveResult::Failed(self); };

        self.focus.no_highlight();
        focus.highlight();

        let mut left = self.left.clone();
        left.push(self.focus.clone());
        let children = focus.get_children().unwrap_or(Vec::new());
        let previous = Some(Breadcrumb { zipper: Box::new(self), direction: PrevDir::Left });

        ZipperMoveResult::Success(Zipper { focus, previous, children, left, right })
    }

    pub fn track_back_to_parent(mut self) -> ZipperMoveResult {
        if let Some(mut zip) = self.previous {
            self.focus.no_highlight();
            match zip.direction {
                PrevDir::Parent { .. } => {
                    zip.zipper.focus.highlight();
                    ZipperMoveResult::Success(*zip.zipper)
                },
                _ => {
                    let mut crumb = match zip.zipper.track_back_to_parent() {
                        ZipperMoveResult::Success(z) => z,
                        ZipperMoveResult::Failed(_) =>
                            panic!("shouldn't be able to fail here"),
                    };
                    
                    crumb.focus.highlight();
                    ZipperMoveResult::Success(crumb)
                }
            }
        } else {
            ZipperMoveResult::Failed(self)
        }
    }

    pub fn move_right_or_cousin(self) -> ZipperMoveResult {
        let result = self.move_right();
        match result {
            // first, move right
            ZipperMoveResult::Success(_) => result,
            ZipperMoveResult::Failed(ref zip) => match zip.clone().track_back_to_parent() {
                // if that fails, go to the parent
                ZipperMoveResult::Failed(_) => result.clone(),
                ZipperMoveResult::Success(zip) => match zip.clone().move_right() {
                    // move to the parent's right sibling
                    ZipperMoveResult::Failed(_) => result.clone(),
                    ZipperMoveResult::Success(zip) => match zip.clone().move_to_child(0) {
                        // then move into your right piblings leftmost child
                        ZipperMoveResult::Failed(_) => result.clone(),
                        ZipperMoveResult::Success(zip) => ZipperMoveResult::Success(zip),
                    }
                }
            }
        }
    }

    pub fn move_left_or_cousin(self) -> ZipperMoveResult {
        let result = self.move_left();
        match result {
            // first, move left
            ZipperMoveResult::Success(_) => result,
            ZipperMoveResult::Failed(ref zip) => match zip.clone().track_back_to_parent() {
                // if that fails, go to the parent
                ZipperMoveResult::Failed(_) => result,
                ZipperMoveResult::Success(mut zip) => match zip.clone().move_left() {
                    // move to the parent's left sibling
                    ZipperMoveResult::Failed(_) => {
                        zip.focus.no_highlight();
                        let mut rv = result.unwrap();
                        rv.focus.highlight();
                        ZipperMoveResult::Failed(rv)
                    },
                    ZipperMoveResult::Success(mut zip) => {
                        match zip.clone().move_to_child(zip.children.len()) {
                            // then move into your left piblings rightmost child
                            ZipperMoveResult::Failed(_) => {
                                zip.focus.no_highlight();
                                let mut rv = result.unwrap();
                                rv.focus.highlight();
                                ZipperMoveResult::Failed(rv)
                            },
                            ZipperMoveResult::Success(zip) => ZipperMoveResult::Success(zip),
                        }
                    }
                }
            }
        }
    }

    pub fn add_child(mut self, node: Node, index: usize) -> Zipper {
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

    pub fn replace_focus(mut self, new_node: Node) -> Zipper {
        self.children = new_node.get_children().unwrap_or(Vec::new());
        self.focus = new_node;
        self
    }
}

