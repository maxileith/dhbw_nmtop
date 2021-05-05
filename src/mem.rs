use std::fs::File;
use std::sync::mpsc;
use std::thread;
use std::time;
use std::{io::BufRead, io::BufReader};
use termion::event::Key;
use tui::{
    backend::Backend,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    terminal::Frame,
    widgets::{Block, Gauge},
};

#[derive(Default, Debug)]
pub struct MemInfo {
    pub mem_total: u32,
    pub mem_free: u32,
    pub mem_available: u32,
    pub swap_total: u32,
    pub swap_free: u32,
    pub swap_cached: u32,
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
    let dur = time::Duration::from_millis(500);

    // Thread for the data collection
    thread::spawn(move || loop {
        let m = match show_ram_usage() {
            Ok(a) => a,
            Err(_) => Default::default(),
        };

        let _ = tx.send(m);

        thread::sleep(dur);
    });

    rx
}

const SIZES: [&str; 4] = ["K", "M", "G", "T"];

pub fn calc_ram_to_fit_size(mem_size: u32) -> String {
    let mut count = 0;

    if mem_size == 0 {
        return "0 bytes".to_string();
    }

    let mut size = mem_size as f64;

    while size > 1000.0 {
        size = size / 1024.0;
        count += 1;
    }

    let size_string: String = format!("{:.1}", size);
    /*if size > 10.0 {
        size_string = format!("{:.0}", size);
    } else {
        size_string = format!("{:.1}", size);
    }*/

    size_string + SIZES[count]
}

pub struct MemoryWidget {
    mem_info: MemInfo,
    dc_thread: mpsc::Receiver<MemInfo>,
}

impl MemoryWidget {
    pub fn new() -> Self {
        Self {
            mem_info: Default::default(),
            dc_thread: init_data_collection_thread(),
        }
    }

    pub fn update(&mut self) {
        // Recv data from the data collector thread

        let mem_info = self.dc_thread.try_recv();

        if mem_info.is_ok() {
            self.mem_info = mem_info.unwrap();
        }
    }

    pub fn draw<B: Backend>(&self, f: &mut Frame<B>, rect: Rect, block: Block) {
        let block_chunks = Layout::default()
            .constraints([Constraint::Length(2), Constraint::Length(2)])
            .margin(1)
            .split(rect);

        // Render block
        f.render_widget(block, rect);

        if self.mem_info.mem_total == 0 || self.mem_info.swap_total == 0 {
            return;
        }

        // calc mem infos
        let mem_usage = ((self.mem_info.mem_total - self.mem_info.mem_available) as f64)
            / (self.mem_info.mem_total as f64);
        let mem_swap = self.mem_info.swap_cached as f64 / self.mem_info.swap_total as f64;
        let label_mem = format!("{:.2}%", mem_usage * 100.0);
        let title_mem = "Memory: ".to_string()
            + &calc_ram_to_fit_size(self.mem_info.mem_total - self.mem_info.mem_available)
            + " of "
            + &calc_ram_to_fit_size(self.mem_info.mem_total);
        let gauge_mem = Gauge::default()
            .block(Block::default().title(title_mem))
            .gauge_style(
                Style::default()
                    .fg(Color::Cyan)
                    .bg(Color::Black)
                    .add_modifier(Modifier::ITALIC | Modifier::BOLD),
            )
            .label(label_mem)
            .ratio(mem_usage);
        f.render_widget(gauge_mem, block_chunks[0]);

        let label_swap = format!("{:.2}%", mem_swap * 100.0);
        let title_swap = "Swap: ".to_string()
            + &calc_ram_to_fit_size(self.mem_info.swap_total - self.mem_info.swap_free)
            + " of "
            + &calc_ram_to_fit_size(self.mem_info.swap_total);
        let gauge_swap = Gauge::default()
            .block(Block::default().title(title_swap))
            .gauge_style(
                Style::default()
                    .fg(Color::Cyan)
                    .bg(Color::Black)
                    .add_modifier(Modifier::ITALIC | Modifier::BOLD),
            )
            .label(label_swap)
            .ratio(mem_swap);
        f.render_widget(gauge_swap, block_chunks[1]);
    }

    pub fn handle_input(&mut self, key: Key) {}
}
