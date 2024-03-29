use tokio::{sync::{mpsc::UnboundedReceiver, RwLock}, task::JoinHandle};

use crate::{input::Command, primatives::Root, zipper::{DynZipper, RootZipper}, State};


pub fn control_thread_init(
    state: &'static RwLock<State>,
    root: &'static RwLock<Root>,
    mut input_rx: UnboundedReceiver<Command>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut zipper: DynZipper = Box::new(RootZipper::new(root).await);

        while let Some(msg) = input_rx.recv().await {
            match msg {
                Command::Insert(_) => (),
                Command::NormalMode => *state.write().await = State::Normal,
                Command::InsertMode => *state.write().await = State::Insert,
                Command::TravelMode => *state.write().await = State::Travel,
                Command::ToFirstChild => zipper = zipper.child(0).await,
                Command::ToParent => zipper = zipper.parent().await,
                Command::ToLeftSibling => {
                    // clear_tx.send(ClearScreenMsg).unwrap();
                    zipper = zipper.move_left().await
                },
                Command::ToRightSibling => {
                    // clear_tx.send(ClearScreenMsg).unwrap();
                    zipper = zipper.move_right().await
                },
                Command::Reset => (),
                Command::ShutDown => *state.write().await = State::ShutDown,
                Command::PrevChar => (),
                Command::PrevLine => (),
                Command::NextLine => (),
                Command::NextChar => (),
                Command::ToLastChild => (),
                Command::ToMiddleChild => (),
            }
        }
    })
}
