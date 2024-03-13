use std::sync::Arc;
use async_trait::async_trait;

use tokio::{sync::{RwLock, RwLockReadGuard, RwLockWriteGuard}, task};

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum FlipFlopFlag {
    Flip,
    Flop
}

#[async_trait]
trait FlipFlopWrite<T: Clone + Send + Sync> {
    async fn write(&self) -> FlipFlopWriteGuard<T>;
}

#[async_trait]
impl<T: Clone + Send + Sync> FlipFlopWrite<T> for Arc<FlipFlop<T>> {
    async fn write(&self) -> FlipFlopWriteGuard<T> {
        match *self.flag.read().await {
            FlipFlopFlag::Flip => {
                let flop = self.flop.write().await;
                FlipFlopWriteGuard(self.clone(), Some(flop))
            }, 
            FlipFlopFlag::Flop => {
                let flip = self.flip.write().await;
                FlipFlopWriteGuard(self.clone(), Some(flip))
            }, 
        }
    }
}

pub struct FlipFlop<T: Clone + Send + Sync> {
    flip: RwLock<T>,
    flop: RwLock<T>,
    flag: RwLock<FlipFlopFlag>,
}

impl<T: Clone + Send + Sync> FlipFlop<T> {
    pub fn new(input: T) -> Arc<FlipFlop<T>> {
        Arc::new(FlipFlop {
            flip: RwLock::new(input.clone()),
            flop: RwLock::new(input.clone()),
            flag: RwLock::new(FlipFlopFlag::Flip).into(),
        })
    }

    pub async fn read(&self) -> FlipFlopReadGuard<T> {
        match *self.flag.read().await {
            FlipFlopFlag::Flip => self.flip.read().await.into(), 
            FlipFlopFlag::Flop => self.flop.read().await.into(), 
        }
    }
}

pub struct FlipFlopReadGuard<'a, T: Clone + Send + Sync>(RwLockReadGuard<'a, T>);

pub struct FlipFlopWriteGuard<'a, T: Clone + Send + Sync>(Arc<FlipFlop<T>>, Option<RwLockWriteGuard<'a, T>>);

impl<'a, T: Clone + Send + Sync> Drop for FlipFlopWriteGuard<'a, T> {
    fn drop(&mut self) {
        task::spawn(async move {
            let rg = self.0.flag.read().await;
            let flipflopflag = *rg;
            drop(rg);

            // drop the write guard to avoid deadlock
            self.1 = None;

            if flipflopflag == FlipFlopFlag::Flip {
                // change flag, diverting new reads to the latest updated data
                *self.0.flag.write().await = FlipFlopFlag::Flop;
                // manually grab the first write lock to the old data
                let mut write_flip = self.0.flip.write().await;
                // get a read on the new data
                let read_flop = self.0.read().await.0;
                // update old data with new
                *write_flip = read_flop.clone();
                // drop guards
                drop(write_flip);
                drop(read_flop);
            } else {
                *self.0.flag.write().await = FlipFlopFlag::Flip;
                let mut write_flop = self.0.flop.write().await;
                let read_flip = self.0.read().await.0;
                *write_flop = read_flip.clone();
                drop(write_flop);
                drop(read_flip);
            }
        });
    }
}


impl<'a, T: Clone + Send + Sync>
    Into<FlipFlopReadGuard<'a, T>> for RwLockReadGuard<'a, T>
{
    fn into(self) -> FlipFlopReadGuard<'a, T> {
        FlipFlopReadGuard(self)
    }
}

