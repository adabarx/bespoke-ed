use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use ratatui::style::{Color, Style};

use crate::{primatives::{Char, Line, Span}, Content, Layout};


type RC<T> = Rc<RefCell<T>>;

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

// type Obj = RC<dyn TreeObj>;
//
// trait TreeObj {
//     fn get_children(&self) -> Option<Vec<Obj>>;
//     fn replace(&mut self);
// }

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

    pub fn get_children(&self) -> Option<Vec<Node>> {
        // returns None if node doesn't carry children
        // returns an empty vec if the node can carry
        // children but currently doesn't
        match self {
            Node::Layout(layout) => match layout.borrow().clone() { // TODO: get rid of this clone
                Layout::Content(content) => match content {
                    Content::FileExplorer { .. } => todo!(),
                    Content::Editor { text, .. } => Some(
                        text.lines.iter()
                            .map(|l| Node::Line(l.clone()))
                            .collect()
                    )
                },
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
                ZipperMoveResult::Failed(_) => result.clone(),
                ZipperMoveResult::Success(zip) => match zip.clone().move_left() {
                    // move to the parent's left sibling
                    ZipperMoveResult::Failed(_) => result.clone(),
                    ZipperMoveResult::Success(zip) => match zip.clone().move_to_child(zip.children.len()) {
                        // then move into your left piblings rightmost child
                        ZipperMoveResult::Failed(_) => result.clone(),
                        ZipperMoveResult::Success(zip) => ZipperMoveResult::Success(zip),
                    }
                }
            }
        }
    }

    pub fn replace_focus(mut self, new_node: Node) -> Zipper {
        self.children = new_node.get_children().unwrap_or(Vec::new());
        self.focus = new_node;
        self
    }
}

