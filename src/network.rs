use std::fs::File;
use std::sync::mpsc;
use std::thread;
use std::time;
use std::{io::BufRead, io::BufReader};

const PROC_NET_DEV: &str = "/proc/net/dev";

#[derive(Default, Debug)]
pub struct NetworkInfo {
    pub interface: String,
    pub rec_bytes: u64,
    pub rec_packets: u64,
    pub rec_errs: u64,
    pub rec_drop: u64,
    pub send_bytes: u64,
    pub send_packets: u64,
    pub send_errs: u64,
    pub send_drop: u64,
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

        if row_values[1].parse::<u64>().unwrap() > network_info.rec_bytes {
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
