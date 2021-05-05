use std::process::Command;
use std::str;
use std::sync::mpsc;
use std::thread;
use std::time;
use termion::{event::Key};

use tui::{
    backend::{Backend},
    layout::{Constraint, Rect},
    style::{Color, Style},
    terminal::Frame,
    widgets::{
        Block, Cell, Row,
        Table,
    },
};

// equals the "df"-command output
#[derive(Debug, Default)]
pub struct DiskInfo {
    pub filesystem: String,
    pub total: usize,
    pub used: usize,
    pub available: usize,
    pub used_percentage: String,
    pub mountpoint: String,
}

pub fn get_disks_usage() -> Vec<DiskInfo> {
    let mut disk_array = Vec::new();
    // execute "df"
    let mut df_command = Command::new("df");
    let df_output = match df_command.output() {
        Ok(x) => x,
        _ => panic!("Could not read df output"),
    };

    // parse string from utf8 Vec
    let df_output_string = match str::from_utf8(&df_output.stdout) {
        Ok(v) => v,
        Err(e) => panic!("Invalid UTF-8 sequence: {}", e),
    };

    // add disks to array
    for line in df_output_string.lines() {
        //println!("{}", line.starts_with("/dev/"));

        if line.starts_with("/dev/") {
            let mut sliced_line = line.split_whitespace();
            let disk_info = DiskInfo {
                filesystem: sliced_line.next().unwrap().replace("/dev", "").to_string(),
                total: sliced_line.next().unwrap().parse().unwrap(),
                used: sliced_line.next().unwrap().parse().unwrap(),
                available: sliced_line.next().unwrap().parse().unwrap(),
                used_percentage: sliced_line.next().unwrap().to_string(),
                mountpoint: sliced_line.next().unwrap().to_string(),
            };

            disk_array.push(disk_info);
        }
    }
    //println!("{:?}", disk_array);

    disk_array
}

pub fn init_data_collection_thread() -> mpsc::Receiver<Vec<DiskInfo>> {
    let (tx, rx) = mpsc::channel();
    let dur = time::Duration::from_millis(500);

    // Thread for the data collection
    thread::spawn(move || loop {
        let m = get_disks_usage();

        let _ = tx.send(m);

        thread::sleep(dur);
    });

    rx
}

const SIZES: [&str; 4] = ["K", "M", "G", "T"];

pub fn calc_disk_size(disk_size: usize) -> String {
    let mut count = 0;

    if disk_size == 0 {
        return "0".to_string();
    }

    let mut size = disk_size as f64;
    size *= 1.024;

    while size > 1000.0 {
        size = size / 1000.0;
        count += 1
    }

    let size_string: String = format!("{:.1}", size);
    /*if size > 10.0 {
        size_string = format!("{:.0}", size);
    } else {
        size_string = format!("{:.1}", size);
    }*/

    size_string + SIZES[count]
}

pub struct DiskWidget {
    disk_info: std::vec::Vec<DiskInfo>,
    dc_thread: mpsc::Receiver<Vec<DiskInfo>>, 
}

impl DiskWidget {
    pub fn new() -> Self {
        Self {
            disk_info: Default::default(),
            dc_thread: init_data_collection_thread(),
        }
    }

    pub fn update(&mut self) {
        // Recv data from the data collector thread
        
        let result = self.dc_thread.try_recv();

        if result.is_ok() {
            self.disk_info = result.unwrap();
        }
    }

    pub fn draw<B: Backend>(&self,
        f: &mut Frame<B>,
        rect: Rect,
        block: Block,
    ) {
        //draw disk info TODO: divide into own function
        let header_cells = ["Partition", "Available", "In Use", "Total", "Used", "Mount"]
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().fg(Color::White)));
        let header = Row::new(header_cells).height(1);
        let rows = self.disk_info.iter().map(|disk| {
            let mut cells = Vec::new();
            cells.push(Cell::from(disk.filesystem.clone()));
            cells.push(Cell::from(calc_disk_size(disk.available)));
            cells.push(Cell::from(calc_disk_size(disk.used)));
            cells.push(Cell::from(calc_disk_size(disk.total)));
            cells.push(Cell::from(disk.used_percentage.clone()));
            cells.push(Cell::from(disk.mountpoint.clone()));
            Row::new(cells)
        });
        let sizing = &size_columns(rect.width);
        let table = Table::new(rows)
            .header(header)
            .block(block)
            .widths(sizing)
            .column_spacing(2);
        f.render_widget(table, rect);
    }

    pub fn handle_input(&mut self, key: Key) {
        /*match key {
            Key::Up => app.show_all_cores = !app.show_all_cores,
            Key::Down => app.show_all_cores = !app.show_all_cores,
            _ => {},
        };*/
    }
}

fn size_columns(area_width: u16) -> Vec<Constraint> {
    let width = area_width - 2;
    if width >= 39 + 10 {
        vec![
            Constraint::Length(9),
            Constraint::Length(9),
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Length(4),
            Constraint::Min(5),
        ]
    } else if width >= 34 + 8 {
        vec![
            Constraint::Length(9),
            Constraint::Length(9),
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Length(4),
        ]
    } else if width >= 30 + 6 {
        vec![
            Constraint::Length(9),
            Constraint::Length(9),
            Constraint::Length(6),
            Constraint::Length(6),
        ]
    } else if width >= 24 + 4 {
        vec![
            Constraint::Length(9),
            Constraint::Length(9),
            Constraint::Length(6),
        ]
    } else if width >= 18 + 2 {
        vec![Constraint::Percentage(50), Constraint::Percentage(50)]
    } else if width >= 9 {
        vec![Constraint::Length(9)]
    } else {
        vec![]
    }
}
