use std::sync::{Arc, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
use anyhow::Result;

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum FlipFlopFlag {
    Flip,
    Flop
}

trait FlipFlopWrite<T: Clone + Send + Sync> {
    fn write(&self) -> FlipFlopWriteGuard<T>;
}

impl<T: Clone + Send + Sync> FlipFlopWrite<T> for Arc<FlipFlop<T>> {
    fn write(&self) -> FlipFlopWriteGuard<T> {
        match *self.flag.read().unwrap() {
            FlipFlopFlag::Flip => {
                let flop = self.flop.write().unwrap();
                FlipFlopWriteGuard(self.clone(), Some(flop))
            }, 
            FlipFlopFlag::Flop => {
                let flip = self.flip.write().unwrap();
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

    pub fn read(&self) -> FlipFlopReadGuard<T> {
        match *self.flag.read().unwrap() {
            FlipFlopFlag::Flip => self.flip.read().unwrap().into(), 
            FlipFlopFlag::Flop => self.flop.read().unwrap().into(), 
        }
    }
}

pub struct FlipFlopReadGuard<'a, T: Clone + Send + Sync>(RwLockReadGuard<'a, T>);

pub struct FlipFlopWriteGuard<'a, T: Clone + Send + Sync>(Arc<FlipFlop<T>>, Option<RwLockWriteGuard<'a, T>>);

impl<'a, T: Clone + Send + Sync> Drop for FlipFlopWriteGuard<'a, T> {
    fn drop(&mut self) {
        // get flag
        let rg = self.0.flag.read().unwrap();
        let flipflopflag = *rg;
        drop(rg);

        // drop the write guard to avoid deadlock
        self.1 = None;

        if flipflopflag == FlipFlopFlag::Flip {
            // change flag, diverting new reads to the latest updated data
            *self.0.flag.write().unwrap() = FlipFlopFlag::Flop;
            // manually grab the first write lock to the old data
            let mut write_flip = self.0.flip.write().unwrap();
            // get a read on the new data
            let read_flop = self.0.read().0;
            // update old data with new
            *write_flip = read_flop.clone();
            // drop guards
            drop(write_flip);
            drop(read_flop);
        } else {
            *self.0.flag.write().unwrap() = FlipFlopFlag::Flip;
            let mut write_flop = self.0.flop.write().unwrap();
            let read_flip = self.0.read().0;
            *write_flop = read_flip.clone();
            drop(write_flop);
            drop(read_flip);
        }
    }
}


impl<'a, T: Clone + Send + Sync>
    Into<FlipFlopReadGuard<'a, T>> for RwLockReadGuard<'a, T>
{
    fn into(self) -> FlipFlopReadGuard<'a, T> {
        FlipFlopReadGuard(self)
    }
}

