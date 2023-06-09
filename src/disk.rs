use std::process::Command;
use std::str;
use std::sync::mpsc;
use std::thread;
use std::time;
use termion::event::Key;

use tui::{
    backend::Backend,
    layout::{Constraint, Rect},
    style::{Color, Style},
    terminal::Frame,
    widgets::{Block, Cell, Row, Table},
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

/// Get current disk usage
/// 
/// This function returns a Vector containing a DiskInfo for each disk.
/// 
/// See ( https://en.wikipedia.org/wiki/Df_(Unix) ) for mor informations on the "df" command.
/// 
/// # Panic
/// 
/// This function will panic if the "df" output is not ok or the "df" output could not be parsed.
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

        if line.starts_with("/dev/") {
            let mut sliced_line = line.split_whitespace();
            // create new DiskInfo while iterating through a line
            // has to be changed when the output of the "df" command changes its order
            let disk_info = DiskInfo {
                filesystem: match sliced_line.next() {
                    Some(x) => x.replace("/dev", "").to_string(),
                    _ => "".to_string(),
                },
                // maybe usecase for unwarp_or_default?
                total: match sliced_line.next() {
                    Some(x) => match x.parse() {
                        Ok(x) => x,
                        _ => 0,
                    },
                    _ => 0,
                },
                used: match sliced_line.next() {
                    Some(x) => match x.parse() {
                        Ok(x) => x,
                        _ => 0,
                    },
                    _ => 0,
                },
                available: match sliced_line.next() {
                    Some(x) => match x.parse() {
                        Ok(x) => x,
                        _ => 0,
                    },
                    _ => 0,
                },
                used_percentage: match sliced_line.next() {
                    Some(x) => x.to_string(),
                    _ => "".to_string(),
                },
                mountpoint: match sliced_line.next() {
                    Some(x) => x.to_string(),
                    _ => "".to_string(),
                },
            };

            disk_array.push(disk_info);
        }
    }

    disk_array
}

/// Initializes a thread to collect and send the disk usage eacht 0.5 seconds.
/// 
/// # Panic
/// 
/// This function won't panic.
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
/// Calculates the disk size to fit decimal metrics.
/// 
/// See https://en.wikipedia.org/wiki/Df_(Unix) for more information on block-sizes.
/// 
/// # Arguments
/// 
/// * 'disk_size' - The count of 1K-blocks or 1024-byte-units
/// 
/// # Panic
/// 
/// This function won't panic.
pub fn calc_disk_size(disk_size: usize) -> String {
    let mut count = 0;

    if disk_size == 0 {
        return "0".to_string();
    }

    let mut size = disk_size as f64;
    // the blocks are not 1k but 1024byte-units
    size *= 1.024;

    // calculate the Size to match the gnome system monitor -> decimal base
    while size > 1000.0 {
        size = size / 1000.0;
        count += 1
    }

    // format size for ui
    let size_string: String = format!("{:.1}", size);
    // append metric
    size_string + SIZES[count]
}

pub struct DiskWidget {
    item_index: usize,
    disk_info: std::vec::Vec<DiskInfo>,
    dc_thread: mpsc::Receiver<Vec<DiskInfo>>,
}

impl DiskWidget {
    /// Returns a new DiskWidget with default values and a new data thread.
    /// 
    /// # Panic
    /// 
    /// This funxtion won't panic.
    pub fn new() -> Self {
        Self {
            item_index: 0,
            disk_info: Default::default(),
            dc_thread: init_data_collection_thread(),
        }
    }
    /// Updates the disk_info of the DiskWidget
    /// 
    /// # Panic
    /// 
    /// This function won't panic.
    pub fn update(&mut self) {
        // Recv data from the data collector thread

        let result = self.dc_thread.try_recv();

        if result.is_ok() {
            self.disk_info = result.unwrap();
        }
    }
    /// Draws disk information in a given Rect.
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
    /// This function draws the DiskWidget based on its disk_info.
    /// Call the update function before to get current information.
    pub fn draw<B: Backend>(&self, f: &mut Frame<B>, rect: Rect, block: Block) {
        //draw disk info TODO: divide into own function
        let header_cells = ["Partition", "Available", "In Use", "Total", "Used", "Mount"]
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().fg(Color::White)));
        let header = Row::new(header_cells).height(1);

        let rows = self.disk_info.iter().skip(self.item_index).map(|disk| {
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
    /// Input Handler for the DiskWidget.
    /// 
    /// Enables Table to scroll up and down.
    pub fn handle_input(&mut self, key: Key) {
        match key {
            Key::Down => {
                if self.item_index < self.disk_info.len() - 1 {
                    self.item_index += 1;
                }
            }
            Key::Up => {
                if self.item_index > 0 {
                    self.item_index -= 1;
                }
            }
            _ => {}
        };
    }
}

/// Adjust tablesize to screen resulting in less details on smaller screens.
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
