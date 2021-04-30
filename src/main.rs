use std::io;
use std::vec::Vec;
use std::{thread, time};
use termion::{event::Key, raw::IntoRawMode};
use tui::{
    backend::{Backend, TermionBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    terminal::Frame,
    widgets::{Block, Borders, Cell, Gauge, Row, Table, Widget},
    Terminal,
};

mod util;
use util::InputEvent;

// Module for reading CPU usage data
mod cpu;

// Module for reading memory usage data
mod mem;
use mem::{calc_ram_to_fit_size, MemInfo};

// Module for reading disk usage data
mod disk;
use disk::{calc_disk_size, DiskInfo};

// TODO: user input to stop execution
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Terminal initialization
    let stdout = io::stdout().into_raw_mode()?;
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    // Initialize input handler
    let input_handler = util::InputHandler::new();

    let cpu_dc_thread = cpu::init_data_collection_thread();
    let mem_dc_thread = mem::init_data_collection_thread();
    let disk_dc_thread = disk::init_data_collection_thread();

    let sleep_duration = time::Duration::from_millis(100);

    terminal.clear()?;
    loop {
        let mem_info = match mem_dc_thread.try_recv() {
            Ok(a) => a,
            Err(_) => Default::default(),
        };

        let disk_info = match disk_dc_thread.try_recv() {
            Ok(a) => a,
            Err(_) => Default::default(),
        };
        // Recv data from the data collector thread
        /*let cpu_stats = match cpu_dc_thread.try_recv() {
            Ok(a) => a,
            Err(_) => vec![],
        };*/

        let _ = terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints(
                    [
                        Constraint::Length(6),
                        Constraint::Min(8),
                        Constraint::Length(6),
                    ]
                    .as_ref(),
                )
                .split(f.size());
            let boxes = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                .split(chunks[0]);
            let block1 = Block::default().title("Block 2").borders(Borders::ALL);
            f.render_widget(block1, chunks[1]);
            let block2 = Block::default().title("Block 3").borders(Borders::ALL);
            f.render_widget(block2, chunks[2]);
            draw_meminfo(f, &boxes, &mem_info);

            draw_diskinfo(f, &boxes, &disk_info);
        });

        // Handle events
        match input_handler.next() {
            Ok(InputEvent::Input(input)) => {
                match input {
                    Key::Ctrl('c') => {
                        terminal.clear()?;
                        break;
                    }
                    _ => {}
                };
            }
            Err(_) => {}
        }

        // Sleep
        thread::sleep(sleep_duration);
    }
    Ok(())
}

fn draw_meminfo<B: Backend>(f: &mut Frame<B>, boxes: &Vec<Rect>, mem_info: &MemInfo) {
    let block_chunks = Layout::default()
        .constraints([Constraint::Length(2), Constraint::Length(2)])
        .margin(1)
        .split(boxes[0]);

    let block = Block::default().title(" Mem ").borders(Borders::ALL);
    f.render_widget(block, boxes[0]);

    // calc mem infos
    let mem_usage =
        ((mem_info.mem_total - mem_info.mem_available) as f64) / (mem_info.mem_total as f64);
    let mem_swap = mem_info.swap_cached as f64 / mem_info.swap_total as f64;
    let label_mem = format!("{:.2}%", mem_usage * 100.0);
    let title_mem = "Memory: ".to_string()
        + &calc_ram_to_fit_size(mem_info.mem_total - mem_info.mem_available)
        + " of "
        + &calc_ram_to_fit_size(mem_info.mem_total);
    let gauge_mem = Gauge::default()
        .block(Block::default().title(title_mem))
        .gauge_style(
            Style::default()
                .fg(Color::Magenta)
                .bg(Color::Black)
                .add_modifier(Modifier::ITALIC | Modifier::BOLD),
        )
        .label(label_mem)
        .ratio(mem_usage);
    f.render_widget(gauge_mem, block_chunks[0]);
    let label_swap = format!("{:.2}%", mem_swap * 100.0);
    let title_swap = "Swap: ".to_string()
        + &calc_ram_to_fit_size(mem_info.swap_total - mem_info.swap_free)
        + " of "
        + &calc_ram_to_fit_size(mem_info.swap_total);
    let gauge_swap = Gauge::default()
        .block(Block::default().title(title_swap))
        .gauge_style(
            Style::default()
                .fg(Color::Magenta)
                .bg(Color::Black)
                .add_modifier(Modifier::ITALIC | Modifier::BOLD),
        )
        .label(label_swap)
        .ratio(mem_swap);
    f.render_widget(gauge_swap, block_chunks[1]);
}

fn draw_diskinfo<B: Backend>(f: &mut Frame<B>, boxes: &Vec<Rect>, disk_info: &Vec<DiskInfo>) {
    //draw disk info TODO: divide into own function
    let block = Block::default().title(" Disks ").borders(Borders::ALL);
    let header_cells = [
        "Partition",
        "Available",
        "In Use",
        "Total",
        "Used",
        "Mount",
    ]
    .iter()
    .map(|h| Cell::from(*h).style(Style::default().fg(Color::White)));
    let header = Row::new(header_cells).height(1);
    let rows = disk_info.iter().map(|disk| {
        let mut cells = Vec::new();
        cells.push(Cell::from(disk.filesystem.clone()));
        cells.push(Cell::from(calc_disk_size(disk.available)));
        cells.push(Cell::from(calc_disk_size(disk.used)));
        cells.push(Cell::from(calc_disk_size(disk.total)));
        cells.push(Cell::from(disk.used_percentage.clone()));
        cells.push(Cell::from(disk.mountpoint.clone()));
        Row::new(cells)
    });
    let sizing = &size_columns(boxes[1].width);
    let table = Table::new(rows)
        .header(header)
        .block(block)
        .widths(sizing)
        .column_spacing(2);

    f.render_widget(table, boxes[1]);
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
        vec![
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ]
    } else if width >= 9 {
        vec![
            Constraint::Length(9),
        ]
    } else {
        vec![]
    }
}
