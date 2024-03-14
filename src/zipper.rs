use async_trait::async_trait;
use crate::{primatives::AsyncWidget, TokioRW};
use tokio::sync::{RwLockWriteGuard, RwLockReadGuard};
use anyhow::Result;

type DynZipper = Box<dyn Zipper<dyn AsyncWidget>>;
type DynWidget = TokioRW<dyn AsyncWidget>;

#[async_trait]
pub trait Zipper<T: AsyncWidget> {
    async fn focus_read(&self) -> RwLockReadGuard<T>;
    async fn focus_write(&self) -> RwLockWriteGuard<T>;

    async fn go_back(self) -> Option<Breadcrumb>;
    async fn go_to_parent(self) -> Move;

    async fn go_left(self) -> Move;
    async fn go_right(self) -> Move;

    async fn go_to_child(self) -> Move;
    async fn update_children(self) -> Result<()>;
}

#[derive(PartialEq, Eq)]
enum PrevDir {
    Left,
    Right,
    Parent,
}

enum Move {
    Passed(DynZipper),
    Blocked(DynZipper)
}

struct Breadcrumb {
    zipper: DynZipper,
    direction: PrevDir,
}

pub struct RootZipper<T: AsyncWidget> {
    focus: TokioRW<T>,
    children: Vec<DynWidget>
}

pub struct LeafZipper<T: AsyncWidget> {
    previous: Breadcrumb,
    focus: TokioRW<T>,
    left: Vec<DynWidget>,
    right: Vec<DynWidget>,
}

pub struct BranchZipper<T: AsyncWidget> {
    previous: Breadcrumb,
    focus: TokioRW<T>,
    left: Vec<DynWidget>,
    right: Vec<DynWidget>,
    children: Vec<DynWidget>
}

impl<T: AsyncWidget> Zipper<T> for BranchZipper<T> {
    async fn focus_read(&self) -> RwLockReadGuard<T> {
        self.focus.read().await
    }

    async fn focus_write(&self) -> RwLockWriteGuard<T> {
        self.focus.write().await
    }

    async fn go_back(self) -> Option<Breadcrumb> {
        Some(self.previous)
    }

    async fn go_to_parent(self) -> Move {
        let mut curr: Box<DynZipper> = Box::new(self);
        while let Some(prev) = self.go_back().await {
            if prev.direction == PrevDir::Parent { break }
            curr = prev.zipper;
        }
        Move::Passed(Box::new(self))
    }

    async fn go_left(self) -> Move {
        if self.previous.direction == PrevDir::Left {
            return Move::Passed(self.previous.zipper);
        }
        let curr_index = self.left.len();
        if curr_index == 0 { return Move::Blocked(self) }

        let focus = self.left.last().unwrap();
        let left = self.left[0..curr_index - 1].iter().cloned().collect();
        let mut right = vec![self.focus.clone()];
        right.extend(self.right.iter().cloned());
        let children = focus.read().await.get_children().await;

        Move::Passed(Box::new(
            BranchZipper {
                previous: Breadcrumb { zipper: Box::new(self), direction: PrevDir::Right },
                focus,
                left,
                right,
                children,
            }
        ))
    }
    async fn go_right(self) -> Move {
        if self.previous.direction == PrevDir::Right {
            return Move::Passed(self.previous.zipper);
        }
        if self.right.len() == 0 { return Move::Blocked(self); }

        let focus = self.right[0].clone();
        let right = self.right[1..].iter().cloned().collect();
        let mut left = self.left.clone();
        left.push(self.focus.clone());
        let children = focus.read().await.get_children().await;

        Move::Passed(Box::new(
            BranchZipper {
                previous: Breadcrumb { zipper: Box::new(self), direction: PrevDir::Left },
                focus,
                left,
                right,
                children,
            }
        ))
    }

    async fn go_to_child(self) -> Move {

    }
    async fn update_children(self) -> Result<()> {}
}




