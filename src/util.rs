use std::io;
use std::process::Command;
use std::sync::mpsc;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{thread, time::Instant};
use termion::event::Key;
use termion::input::TermRead;

/// Stores the receiving end of a channel to read keyboard events.
pub struct InputHandler {
    rx: mpsc::Receiver<Key>,
}

impl InputHandler {
    /// Create a new channel and read keyboard events from stdin.
    /// The keyboard events are only sent at certain intervals to the receiving end, other events
    /// are discarded.
    /// The interval is necessary to prevent the flooding of the receiver with events since the 
    /// receiver may need some processing time.
    /// Some caveats of this approach are the input lag in text fields and occasionaly a key press
    /// is not detected.
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(); // create a channel for thread communication

        thread::spawn(move || {
            let stdin = io::stdin();
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

    /// Tries to fetch new event from channel
    pub fn next(&self) -> Result<Key, mpsc::TryRecvError> {
        self.rx.try_recv()
    }
}

const SIZES: [&str; 5] = [" B", " KiB", " MiB", " GiB", " TiB"];

/// Convert bytes to human readable format.
/// Values are displayed as B, KiB, MiB, GiB or TiB
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

/// Send a kill signal to a process selected by the pid.
pub fn kill_process(pid: usize) {
    let pid_string = &pid.to_string();
    Command::new("kill")
        .args(&["-9", pid_string])
        .output()
        .expect("failed to kill process");
}

/// Update the niceness of a process.
/// Niceness can be increaesd with normal user privileges.
/// Sudo privileges are required by Linux to downgrade the niceness of a process.
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

/// Get the millis since Day 0 (01.01.1970)
///
/// # Panic
///
/// This function won't panic.
pub fn get_millis() -> usize {
    let tmp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    tmp.as_secs() as usize * 1000 + tmp.subsec_nanos() as usize / 1_000_000 as usize
}
