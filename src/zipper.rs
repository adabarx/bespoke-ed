//time 2 rewrite
use std::{cmp::min, collections::VecDeque, marker::PhantomData, sync::Arc};

use ratatui::style::{Color, Style};
use async_trait::async_trait;
use either::*;
use tokio::sync::RwLock;

use crate::{primatives::{AsyncWidget, Char, Layout, LayoutType, Line, Span, Text}, ARW};

#[async_trait]
pub trait TreeZipper<Focus, Child, Parent> {
    async fn get_focus(&self) -> ARW<Focus>;

    async fn prev(&self) -> Option<Either<Focus, Parent>> { None }
    async fn mother(&self) -> Option<Either<Focus, Parent>> { None }
    async fn daughter(&self, index: usize) -> Option<Either<Focus, Child>> { None }

    async fn left_sister(&self) -> Option<Either<Focus, Focus>> { None }
    async fn right_sister(&self) -> Option<Either<Focus, Focus>> { None }

    async fn left_aunt(&self) -> Option<Either<Focus, Parent>> { None }
    async fn right_aunt(&self) -> Option<Either<Focus, Parent>> { None }

    async fn left_cousin(&self, index: usize) -> Option<Either<Focus, Focus>> { None }
    async fn right_cousin(&self, index: usize) -> Option<Either<Focus, Focus>> { None }

    async fn left_sister_or_cousin(&self) -> Option<Either<Focus, Focus>> { None }
    async fn right_sister_or_cousin(&self) -> Option<Either<Focus, Focus>> { None }
}

#[derive(Clone)]
pub enum Node {
    Layout(ARW<Layout>),
    Line(ARW<Line>),
    Span(ARW<Span>),
    Char(ARW<Char>),
}

impl Node {
     fn get_layout(&self) -> Option<ARW<Layout>> {
        if let Node::Layout(layout) = self {
            Some(layout.clone())
        } else {
            None
        }
    }

    // async fn try_add_child(&mut self, child: Node, index: usize) -> Result<Node> {
    //     use Node::*;
    //     match (self, child) {
    //         (Layout(mom), Layout(child)) => Ok(Node::Layout(
    //             (*mom.write().await).try_add_child(child.read().await.clone(), index)?
    //         )),
    //         (Layout(mom), Line(child)) => Ok(Node::Line(
    //             (*mom.write().await).try_add_child(child.read().await.clone(), index)?
    //         )),
    //         (Line(mom), Span(child)) => Ok(Node::Span(
    //             (*mom.write().await).add_child(child.read().await.clone(), index)
    //         )),
    //         (Span(mom), Char(child)) => Ok(Node::Char(
    //             (*mom.write().await).add_child(child.read().await.clone(), index)
    //         )),
    //         _ => Err(anyhow!("this child does not please mother")),
    //     }
    // }

     async fn get_children(&self) -> Option<Vec<Node>> {
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

     async fn highlight(&mut self) {
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

     async fn no_highlight(&mut self) {
        match self {
            Node::Line(line) => line.write().await.style = Style::default(),
            Node::Span(span) => span.write().await.style = Style::default(),
            Node::Char(char) => char.write().await.style = Style::default(),
            Node::Layout(layout) => layout.write().await.style = Style::default(),
        }
    }
}

#[derive(Clone)]
pub enum WindowChild {
    Window(ARW<Layout>),
    Content(ARW<Text>)
}

#[derive(Clone)]
pub struct Zipper<Focus, Child, Parent, ParentZipper, GrandParent, GrandChild>
where
    Focus: AsyncWidget<Child> + Clone + Send + Sync,
    Child: AsyncWidget<GrandChild> + Clone + Send + Sync,
    Parent: AsyncWidget<Focus> + Clone + Send + Sync,
    ParentZipper: TreeZipper<Parent, Focus, GrandParent> + Clone + Send + Sync,
    GrandChild: Clone + Send + Sync
{
    focus: ARW<Focus>,
    parent: Option<Box<Parent>>,
    children: Option<Vec<ARW<Child>>>,
    left: Vec<ARW<Focus>>,
    right: VecDeque<ARW<Focus>>,
    pd: PhantomData<(GrandChild, GrandParent, ParentZipper)>
}

impl<F, C, P, Pz, Gp, Gc> Zipper<F, C, P, Pz, Gp, Gc>
where
    F: AsyncWidget<C> + Clone + Send + Sync,
    C: AsyncWidget<Gc> + Clone + Send + Sync,
    P: AsyncWidget<F> + Clone + Send + Sync,
    Pz: TreeZipper<P, F, Gp> + Clone + Send + Sync,
    Gc: Clone + Send + Sync,
    Gp: Sync
{
    pub async fn new_root(focus: ARW<F>) -> Self {
        let children =
            Some(focus
                .read().await
                .get_children().await
                .unwrap());
        Self {
            focus,
            children,
            left: Vec::new(),
            right: VecDeque::new(),
            parent: None,
            pd: PhantomData,
        }
    }

    pub async fn new_branch(mom: P, index: usize) -> Self {
        let siblings = mom.get_children().await.unwrap();
        let index = min(index, siblings.len());
        let focus = siblings[index].clone();
        let left = siblings[0..index].iter().cloned().collect();
        let right = siblings[index + 1..].iter().cloned().collect();
        let children = focus.read().await.get_children().await;
        Self {
            focus,
            children,
            left,
            right,
            parent: Some(Box::new(mom)),
            pd: PhantomData,
        }
    }
}

#[async_trait]
impl<F, C, P, Pz, Gp, Gc> TreeZipper<F, C, P> for Zipper<F, C, P, Pz, Gp, Gc>
where
    F: AsyncWidget<C> + Clone + Send + Sync,
    C: AsyncWidget<Gc> + Clone + Send + Sync,
    P: AsyncWidget<F> + Clone + Send + Sync,
    Pz: TreeZipper<P, F, Gp> + Clone + Send + Sync,
    Gc: Clone + Send + Sync,
    Gp: Sync
{
    async fn get_focus(&self) -> ARW<F> {
        self.focus.clone()
    }

    async fn prev(&self) -> Option<Either<F, P>> { None }
    async fn mother(&self) -> Option<Either<F, P>> { None }
    async fn daughter(&self, index: usize) -> Option<Either<F, C>> { None }

    async fn left_sister(&self) -> Option<Either<F, F>> { None }
    async fn right_sister(&self) -> Option<Either<F, F>> { None }

    async fn left_aunt(&self) -> Option<Either<F, P>> { None }
    async fn right_aunt(&self) -> Option<Either<F, P>> { None }

    async fn left_cousin(&self, index: usize) -> Option<Either<F, F>> { None }
    async fn right_cousin(&self, index: usize) -> Option<Either<F, F>> { None }

    async fn left_sister_or_cousin(&self) -> Option<Either<F, F>> { None }
    async fn right_sister_or_cousin(&self) -> Option<Either<F, F>> { None }
}
