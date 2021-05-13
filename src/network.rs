use std::fs::File;
use std::sync::mpsc;
use std::thread;
use std::time;
use std::{io::BufRead, io::BufReader};
use termion::event::Key;
use tui::{
    backend::Backend,
    layout::Rect,
    terminal::Frame,
    text::Spans,
    widgets::{Block, Paragraph, Wrap},
};

use crate::util;

const PROC_NET_DEV: &str = "/proc/net/dev";
// all information which are used or can be used later
#[derive(Default, Debug)]
pub struct NetworkInfo {
    pub interface: String,
    pub rec_bytes: usize,
    pub rec_packets: usize,
    pub rec_errs: usize,
    pub rec_drop: usize,
    pub send_bytes: usize,
    pub send_packets: usize,
    pub send_errs: usize,
    pub send_drop: usize,
}

/// Get the current network I/O
/// 
/// This function reads the current newtwork information from "/proc/net/dev" and returns a Result.
/// The Result is either a NetworkInfo-objet or an Error.
/// 
/// See https://www.kernel.org/doc/html/latest/networking/statistics.html for morte information.
/// 
/// # Panic
/// 
/// This function won't panic.
pub fn get_network_io() -> Result<NetworkInfo, Box<dyn std::error::Error>> {
    let file = File::open(PROC_NET_DEV)?;
    let reader = BufReader::new(file);
    let mut network_info: NetworkInfo = Default::default();

    // read network io info with iterator
    let mut line_iterator = reader.lines();

    // skipping the first two lines containing a description
    line_iterator.next();
    line_iterator.next();
    // filter / skip local network activity
    line_iterator.next();

    for line in line_iterator {
        let row = match line {
            Ok(x) => x,
            _ => break,
        };

        // collect iterator into vector
        let row_values = row.split_whitespace().collect::<Vec<_>>();

        // check for the network adapter with the most incoming trafic -> row_values[1] is the value for total bytes recieved
        // unwrap_or_default, because the default (0) will always be skipped
        if row_values[1].parse::<usize>().unwrap_or_default() > network_info.rec_bytes {
            // unwrap_or_default to match "normal" thread error, where a Default::default will be returned -> DiskWidget handels defaults
            network_info.interface = row_values[0].to_string();
            network_info.rec_bytes = row_values[1].parse().unwrap_or_default();
            network_info.rec_packets = row_values[2].parse().unwrap_or_default();
            network_info.rec_errs = row_values[3].parse().unwrap_or_default();
            network_info.rec_drop = row_values[4].parse().unwrap_or_default();
            network_info.send_bytes = row_values[9].parse().unwrap_or_default();
            network_info.send_packets = row_values[10].parse().unwrap_or_default();
            network_info.send_errs = row_values[11].parse().unwrap_or_default();
            network_info.send_drop = row_values[12].parse().unwrap_or_default();
        }
    }

    Ok(network_info)
}

/// Initializes a thread to collect and send the network information eacht 0.5 seconds.
/// 
/// It will send a NetworkInfo-object with default values if an error occurs in get_network_io.
/// 
/// # Panic
/// 
/// This function won't panic.
pub fn init_data_collection_thread() -> mpsc::Receiver<NetworkInfo> {
    let (tx, rx) = mpsc::channel();
    let dur = time::Duration::from_millis(500);

    // Thread for the data collection
    thread::spawn(move || loop {
        let m = match get_network_io() {
            Ok(a) => a,
            Err(_) => Default::default(),
        };

        let _ = tx.send(m);

        thread::sleep(dur);
    });

    rx
}

pub struct NetworkWidget {
    current_info: NetworkInfo,
    last_info: NetworkInfo,
    dc_thread: mpsc::Receiver<NetworkInfo>,
}

impl NetworkWidget {
    /// Returns a new NetworkWidget with default values and a new data thread.
    /// 
    /// # Panic
    /// 
    /// This function won't panic.
    pub fn new() -> Self {
        Self {
            current_info: Default::default(),
            last_info: Default::default(),
            dc_thread: init_data_collection_thread(),
        }
    }
    /// Updates the current information and rotates the older one
    /// 
    /// # Panic
    /// 
    /// This function won't panic. 
    pub fn update(&mut self) {
        // Recv data from the data collector thread
        let network_info = self.dc_thread.try_recv();

        if network_info.is_ok() {
            self.last_info = NetworkInfo {
                interface: self.current_info.interface.clone(),
                ..self.current_info
            };

            // we network_info is ok / safe at this point
            self.current_info = network_info.unwrap();
        }
    }
    /// Draws all network information in a given Rect.
    /// 
    /// # Arguments
    /// 
    /// * 'f' - A refrence to the terminal interface for rendering
    /// * 'rect' - A rectangle used to hint the area the widget gets rendered in
    /// * 'block' - A Box with borders and title which contains the drawn widget
    /// 
    /// # Panic
    /// 
    /// This function won't panic.
    /// 
    /// # Usage
    /// 
    /// This function draws the NetworkInfo based on current_info and last_info.
    /// Call the update function before to get current information.
    /// 
    /// Call the update and draw function each 0.5seconds to get precise meassurements.
    pub fn draw<B: Backend>(&self, f: &mut Frame<B>, rect: Rect, block: Block) {
        if self.last_info.rec_bytes > self.current_info.rec_bytes {
            return;
        }

        // the factor is based on the refreshing-rate of the ui (500ms)
        let receiving =
            util::to_humanreadable((self.current_info.rec_bytes - self.last_info.rec_bytes) * 2)
                + "/s";
        let sending =
            util::to_humanreadable((self.current_info.send_bytes - self.last_info.send_bytes) * 2)
                + "/s";

        let text: Vec<tui::text::Spans>;
        // adjust information to size, showing less informations on smaller screens
        if rect.width > 25 {
            let total_received = util::to_humanreadable(self.current_info.rec_bytes);
            let total_sent = util::to_humanreadable(self.current_info.send_bytes);
            text = vec![
                Spans::from(format!("Receiving      {}", receiving)),
                Spans::from(format!("Total Received {}", total_received)),
                Spans::from(format!("Sending        {}", sending)),
                Spans::from(format!("Total Sent     {}", total_sent)),
            ];
        } else {
            text = vec![
                Spans::from("Receiving"),
                Spans::from(format!("{}", receiving)),
                Spans::from("Sending"),
                Spans::from(format!("{}", sending)),
            ];
        }

        let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: true });
        f.render_widget(paragraph, rect);
    }

    pub fn handle_input(&mut self, key: Key) {}
}
