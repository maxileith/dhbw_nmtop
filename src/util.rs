use std::io;
use std::sync::mpsc;
use std::thread;
use termion::event::Key;
use termion::input::TermRead;


pub enum InputEvent<I> {
    Input(I),
}

pub struct InputHandler {
    rx: mpsc::Receiver<InputEvent<Key>>,
    input_handle: thread::JoinHandle<()>,
}

impl InputHandler {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(); // create a channel for thread communication

        let input_handle = {
            thread::spawn(move || {
                let stdin = io::stdin();

                for evt in stdin.keys() {
                    if let Ok(key) = evt {
                        tx.send(InputEvent::Input(key));
                    }
                }
            })
        };
        InputHandler {
            rx: rx,
            input_handle: input_handle,
        }
    }

    pub fn next(&self) -> Result<InputEvent<Key>, mpsc::TryRecvError> {
        self.rx.try_recv()
    }
}
