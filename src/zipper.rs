use async_trait::async_trait;
use crate::{primatives::{AsyncWidget, Window, WindowChild}, ARW};
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use anyhow::Result;

#[async_trait]
pub trait Zipper<F, C>
where
    F: AsyncWidget + ?Sized + 'static,
    C: AsyncWidget + ?Sized + 'static,
{
    async fn focus_read(&self) -> RwLockReadGuard<F>;
    async fn focus_write(&self) -> RwLockWriteGuard<F>;

    async fn go_back(self) -> Option<Breadcrumb>;
    async fn go_to_parent(self) -> Move<F, F>;

    async fn go_left(self) -> Move<F, F>;
    async fn go_right(self) -> Move<F, F>;

    async fn go_to_child(self) -> Move<F, C>;
    async fn update_children(self) -> Result<()>;
}

type DynZipper = Box<dyn Zipper<dyn AsyncWidget, dyn AsyncWidget>>;

#[derive(PartialEq, Eq)]
#[derive(Clone)]
enum PrevDir {
    Left,
    Right,
    Parent,
}

enum Move<F, C>
where
    F: AsyncWidget + ?Sized + 'static,
    C: AsyncWidget + ?Sized + 'static,
{
    Parent(DynZipper),
    Child(Box<C>),
    Left(Box<F>),
    Right(Box<F>),
    Blocked(Box<F>),
}

#[derive(Clone)]
struct Breadcrumb {
    zipper: DynZipper,
    direction: PrevDir,
}

pub struct RootZipper
where
{
    root: &'static RwLock<Window>,
    children: Vec<ARW<DynZipper>>
}

impl RootZipper
{
    pub async fn init(root: &'static RwLock<Window>) -> Self {
        let children = root.read().await.windows.clone();
        Self { root, children }
    }
}

pub struct LeafZipper<F> 
where
    F: AsyncWidget + ?Sized + 'static,
{
    previous: Breadcrumb,
    focus: ARW<F>,
    left: Vec<ARW<F>>,
    right: Vec<ARW<F>>,
}

#[derive(Clone)]
pub struct BranchZipper<F, C>
where
    F: AsyncWidget + Clone + ?Sized + 'static + Send + Sync,
    C: AsyncWidget + Clone + ?Sized + 'static + Send + Sync,
{
    previous: Breadcrumb,
    focus: ARW<F>,
    left: Vec<ARW<F>>,
    right: Vec<ARW<F>>,
    children: Vec<ARW<C>>
}

#[async_trait]
impl<F, C> Zipper<F, C> for BranchZipper<F, C>
where
    F: AsyncWidget + Clone + 'static + Send + Sync,
    C: AsyncWidget + Clone + ?Sized + 'static + Send + Sync,
{
    async fn focus_read(&self) -> RwLockReadGuard<F> {
        self.focus.read().await
    }

    async fn focus_write(&self) -> RwLockWriteGuard<F> {
        self.focus.write().await
    }

    async fn go_back(self) -> Option<Breadcrumb> {
        Some(self.previous)
    }

    async fn go_to_parent(self) -> Move<F, F> {
        if self.previous.direction == PrevDir::Parent { Move::Parent(self.previous.zipper) }
        else { self.previous.zipper.go_to_parent() }
    }

    async fn go_left(self) -> Move<F, F> {
        if self.previous.direction == PrevDir::Left {
            return Move::Left(self.previous.zipper);
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
    async fn go_right(self) -> Move<F, F> {
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

    async fn go_to_child(self) -> Move<F, C> {

    }
    async fn update_children(self) -> Result<()> {
        Ok(())
    }
}




