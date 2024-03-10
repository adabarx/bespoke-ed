use std::sync::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
use anyhow::Result;

pub enum FlipFlopFlag {
    Flip,
    Flop
}

pub struct FlipFlop<T: Clone + Send + Sync> {
    flip: RwLock<T>,
    flop: RwLock<T>,
    flag: RwLock<FlipFlopFlag>,
}

impl<T: Clone + Send + Sync> FlipFlop<T> {
    pub fn new(input: T) -> FlipFlop<T> {
        FlipFlop {
            flip: RwLock::new(input.clone()),
            flop: RwLock::new(input.clone()),
            flag: RwLock::new(FlipFlopFlag::Flip)
        }
    }

    pub fn read(&self) -> FlipFlopReadGuard<T> {
        match *self.flag.read().unwrap() {
            FlipFlopFlag::Flip => self.flip.read().unwrap().into(), 
            FlipFlopFlag::Flop => self.flop.read().unwrap().into(), 
        }
    }

    pub fn write(&self) -> FlipFlopWriteGuard<T> {
        match *self.flag.read().unwrap() {
            FlipFlopFlag::Flip => self.flop.write().unwrap().into(), 
            FlipFlopFlag::Flop => self.flip.write().unwrap().into(), 
        }
    }
}

pub struct FlipFlopReadGuard<'a, T: Clone + Send + Sync>(RwLockReadGuard<'a, T>);

pub struct FlipFlopWriteGuard<'a, T: Clone + Send + Sync>(RwLockWriteGuard<'a, T>);

impl<'a, T: Clone + Send + Sync>
    Into<FlipFlopReadGuard<'a, T>> for RwLockReadGuard<'a, T>
{
    fn into(self) -> FlipFlopReadGuard<'a, T> {
        FlipFlopReadGuard(self)
    }
}

impl<'a, T: Clone + Send + Sync>
    Into<FlipFlopWriteGuard<'a, T>> for RwLockWriteGuard<'a, T>
{
    fn into(self) -> FlipFlopWriteGuard<'a, T> {
        FlipFlopWriteGuard(self)
    }
}

