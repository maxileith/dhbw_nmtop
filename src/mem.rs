use std::{io::BufRead, io::BufReader};
use std::fs::File;
use std::time;
use std::thread;
use std::sync::mpsc;

#[derive(Default, Debug)]
pub struct MemInfo {
    pub mem_total: f64,
    pub mem_free: f64,
    pub mem_available: f64,
    pub swap_total: f64,
    pub swap_free: f64,
    pub swap_cached: f64,
}

pub fn show_ram_usage() -> Result<MemInfo, Box<dyn std::error::Error>> {
    let meminfo = "/proc/meminfo";
    
    let mut mem_info: MemInfo = Default::default();
    
    let file = File::open(meminfo)?;
    let reader = BufReader::new(file);
    let mut mem_numbers: [String; 6] = [
        String::from("0"),
        String::from("0"),
        String::from("0"),
        String::from("0"),
        String::from("0"),
        String::from("0"),
    ];
    let mut count = 0;

    for line in reader.lines() {
        let row = match line {
            Ok(x) => x,
            Err(_) => break,
        };
        if row.starts_with("Mem") || row.starts_with("Swap") {
            let mut row_values = row.split_whitespace();

            row_values.next();
            match row_values.next() {
                Some(x) => mem_numbers[count] = x.to_string(),
                None => break,
            }

            count += 1;
        }
    }

    mem_info.mem_total = mem_numbers[0].parse().unwrap();
    mem_info.mem_free = mem_numbers[1].parse().unwrap();
    mem_info.mem_available = mem_numbers[2].parse().unwrap();
    mem_info.swap_cached = mem_numbers[3].parse().unwrap();
    mem_info.swap_total = mem_numbers[4].parse().unwrap();
    mem_info.swap_free = mem_numbers[5].parse().unwrap();

    Ok(mem_info)
}

pub fn init_data_collection_thread() -> mpsc::Receiver<MemInfo> {
    let (tx, rx) = mpsc::channel();
    let dur = time::Duration::from_millis(100);

    // Thread for the data collection
    let dc_thread = thread::spawn(move || loop {
        let m = match show_ram_usage(){
            Ok(a) => a,
            Err(_) => Default::default(),
        };
        
        tx.send(m);

        thread::sleep(dur);
    });

    rx
}

