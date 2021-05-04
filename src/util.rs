use std::io;
use std::sync::mpsc;
use std::thread;
use termion::event::Key;
use termion::input::TermRead;

pub struct InputHandler {
    rx: mpsc::Receiver<Key>,
}

impl InputHandler {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(); // create a channel for thread communication

        thread::spawn(move || {
            let stdin = io::stdin();

            for evt in stdin.keys() {
                if let Ok(key) = evt {
                    let _ = tx.send(key);
                }
            }
        });
        InputHandler { rx: rx }
    }

    pub fn next(&self) -> Result<Key, mpsc::TryRecvError> {
        self.rx.try_recv()
    }
}
