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

pub fn get_network_io() -> Result<NetworkInfo, Box<dyn std::error::Error>> {
    let file = File::open(PROC_NET_DEV)?;
    let reader = BufReader::new(file);
    let mut network_info: NetworkInfo = Default::default();

    let mut line_iterator = reader.lines();

    // skipping the first two lines containing a description
    line_iterator.next();
    line_iterator.next();
    // filter local network activity
    line_iterator.next();

    for line in line_iterator {
        let row = match line {
            Ok(x) => x,
            _ => break,
        };

        // collect iterator into vector
        let row_values = row.split_whitespace().collect::<Vec<_>>();

        if row_values[1].parse::<usize>().unwrap() > network_info.rec_bytes {
            network_info.interface = row_values[0].to_string();
            network_info.rec_bytes = row_values[1].parse().unwrap();
            network_info.rec_packets = row_values[2].parse().unwrap();
            network_info.rec_errs = row_values[3].parse().unwrap();
            network_info.rec_drop = row_values[4].parse().unwrap();
            network_info.send_bytes = row_values[9].parse().unwrap();
            network_info.send_packets = row_values[10].parse().unwrap();
            network_info.send_errs = row_values[11].parse().unwrap();
            network_info.send_drop = row_values[12].parse().unwrap();
        }
    }

    Ok(network_info)
}

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
    pub fn new() -> Self {
        Self {
            current_info: Default::default(),
            last_info: Default::default(),
            dc_thread: init_data_collection_thread(),
        }
    }

    pub fn update(&mut self) {
        // Recv data from the data collector thread
        let network_info = self.dc_thread.try_recv();

        if network_info.is_ok() {
            //FIXME: uglyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy
            self.last_info = NetworkInfo {
                interface: self.current_info.interface.clone(),
                rec_bytes: self.current_info.rec_bytes,
                rec_packets: self.current_info.rec_packets,
                rec_errs: self.current_info.rec_errs,
                rec_drop: self.current_info.rec_drop,
                send_bytes: self.current_info.send_bytes,
                send_packets: self.current_info.send_packets,
                send_errs: self.current_info.send_errs,
                send_drop: self.current_info.send_drop,
            };

            self.current_info = network_info.unwrap();
        }
    }

    pub fn draw<B: Backend>(&self, f: &mut Frame<B>, rect: Rect, block: Block) {
        if self.last_info.rec_bytes > self.current_info.rec_bytes {
            return;
        }

        let receiving =
            util::to_humanreadable((self.current_info.rec_bytes - self.last_info.rec_bytes) * 2)
                + "/s";
        let sending =
            util::to_humanreadable((self.current_info.send_bytes - self.last_info.send_bytes) * 2)
                + "/s";
        let text: Vec<tui::text::Spans>;

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
