use std::io;
use std::process::Command;
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

const SIZES: [&str; 5] = [" byte", " KiB", " MiB", " GiB", " TiB"];

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

pub fn kill_process(pid: usize) {
    let pid_string = &pid.to_string();
    Command::new("kill")
        .args(&["-9", pid_string])
        .output()
        .expect("failed to kill process");
}

pub fn update_niceness(pid: usize, new_niceness: i8) {
    // niceness is measured between -20 and 19
    if new_niceness >= -20 && new_niceness <= 19 {
        let pid_string = &pid.to_string();
        let niceness_string = &new_niceness.to_string();
        Command::new("renice")
            .args(&["-n", niceness_string, "-p", pid_string])
            .output()
            .expect("failed adjust niceness");
    }
}
