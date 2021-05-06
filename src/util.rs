use std::io;
use std::sync::mpsc;
use std::{thread, time::Duration, time::Instant};
use termion::event::Key;
use termion::input::TermRead;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct InputHandler {
    rx: mpsc::Receiver<Key>,
}

impl InputHandler {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(); // create a channel for thread communication

        thread::spawn(move || {
            let stdin = io::stdin();
            let mut previous_key = Key::Null;
            let mut counter = 0;
            let mut previous_time = Instant::now();

            for evt in stdin.keys() {
                let m = previous_time.elapsed().as_millis();

                if m > 150 {
                    if let Ok(key) = evt {
                        let _ = tx.send(key);
                    }
                    previous_time = Instant::now();
                }
            }
        });
        InputHandler { rx: rx }
    }

    pub fn next(&self) -> Result<Key, mpsc::TryRecvError> {
        self.rx.try_recv()
    }
}

const SIZES: [&str; 5] = [" B", " KiB", " MiB", " GiB", " TiB"];

pub fn to_humanreadable(bytes: usize) -> String {
    let mut count = 0;

    if bytes < 1000 {
        return bytes.to_string() + SIZES[count];
    }

    let mut size = bytes as f64;

    while size > 1000.0 {
        size = size / 1024.0;
        count += 1;
    }

    let size_string = format!("{:.1}", size);

    size_string + SIZES[count]
}

pub fn get_millis() -> usize {
    let tmp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    tmp.as_secs() as usize * 1000 + tmp.subsec_nanos() as usize / 1_000_000 as usize
}
