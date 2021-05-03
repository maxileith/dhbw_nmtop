use std::io;
use std::vec::Vec;
use std::{thread, time};
use termion::{event::Key, raw::IntoRawMode};
use tui::{
    backend::{Backend, TermionBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    terminal::Frame,
    text::{Span, Spans},
    widgets::{
        Axis, Block, Borders, Cell, Chart, Dataset, Gauge, GraphType, Paragraph, Row, Table, Wrap,
    },
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

// Module for managing processes
mod processes;
use processes::ProcessList;
// Module for reading network usage
mod network;
use network::{to_humanreadable, NetworkInfo};

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
    let processes_dc_thread = processes::init_data_collection_thread();
    let network_dc_thread = network::init_data_collection_thread();

    let sleep_duration = time::Duration::from_millis(100);

    let mut core_values = Vec::<Vec<f64>>::new();
    let mut cpu_values = Vec::<f64>::new();
    let mut last_network_info: NetworkInfo = Default::default();

    let mut processes_info: ProcessList = Default::default();
    let mut mem_info: MemInfo = Default::default();
    let mut disk_info: std::vec::Vec<disk::DiskInfo> = Default::default();

    //let mut cpu_values = Vec::<f64>::new();
    terminal.clear()?;
    loop {
        mem_info = match mem_dc_thread.try_recv() {
            Ok(a) => a,
            Err(_) => mem_info,
        };

        // Recv data from the data collector thread
        disk_info = match disk_dc_thread.try_recv() {
            Ok(a) => a,
            Err(_) => disk_info,
        };
        // Recv data from the data collector thread
        let cpu_stats = match cpu_dc_thread.try_recv() {
            Ok(a) => a,
            Err(_) => vec![],
        };
        // Recv data from the data collector thread
        processes_info = match processes_dc_thread.try_recv() {
            Ok(a) => a,
            Err(_) => processes_info,
        };

        let network_info = match network_dc_thread.try_recv() {
            Ok(a) => a,
            Err(_) => Default::default(),
        };
        // create cpu info
        let mut counter = 0;
        for b in cpu_stats {
            if b.cpu_name == "cpu" {
                if cpu_values.len() == 300 {
                    cpu_values.remove(0);
                }
                cpu_values.push(b.utilization);
            } else {
                if core_values.len() > counter {
                    if core_values[counter].len() == 300 {
                        core_values[counter].remove(0);
                    }
                    core_values[counter].push(b.utilization);
                } else {
                    core_values.push(Vec::new());
                    core_values[counter].push(b.utilization);
                }
                counter += 1
            }
        }

        let _ = terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints(
                    [
                        Constraint::Length(6),
                        Constraint::Length(10),
                        Constraint::Min(1),
                    ]
                    .as_ref(),
                )
                .split(f.size());
            let boxes = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(30), Constraint::Percentage(45), Constraint::Percentage(25)].as_ref())
                .split(chunks[0]);
            // let block1 = Block::default().title("Block 2").borders(Borders::ALL);
            // f.render_widget(block1, chunks[1]);
            // let block2 = Block::default().title("Block 3").borders(Borders::ALL);
            // f.render_widget(block2, chunks[2]);

            // print!("{:?}", processes_info.processes);

            draw_cpuinfo(f, chunks[1], &cpu_values, &core_values);
            draw_meminfo(f, &boxes, &mem_info);
            draw_diskinfo(f, &boxes, &disk_info);
            draw_processesinfo(f, chunks[2], &processes_info);
            draw_networkinfo(f, boxes[2], &last_network_info, &network_info);
        });

        last_network_info = network_info;

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

    let block = Block::default()
        .title(Span::styled(
        "Memory",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
        )).borders(Borders::ALL);
    f.render_widget(block, boxes[0]);

    if mem_info.mem_total == 0 || mem_info.swap_total == 0 {
        return;
    }

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
                .fg(Color::Cyan)
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
                .fg(Color::Cyan)
                .bg(Color::Black)
                .add_modifier(Modifier::ITALIC | Modifier::BOLD),
        )
        .label(label_swap)
        .ratio(mem_swap);
    f.render_widget(gauge_swap, block_chunks[1]);
}

fn draw_cpuinfo<B: Backend>(f: &mut Frame<B>, rect: Rect, data: &Vec<f64>, cores: &Vec<Vec<f64>>) {
    let mut datasets = Vec::new();

    let mut core_values = Vec::new();
    for core in cores {
        let value = core
            .iter()
            .enumerate()
            .map(|(i, &x)| ((i as f64), x))
            .collect::<Vec<_>>();
        core_values.push(value);
    }

    for i in 0..core_values.len() {
        let h = (i * 40) % 360;
        let mut color = Color::White;
        if h < 60 {
            color = Color::Rgb(255, (h % 255) as u8, 0);
        } else if h < 120 {
            color = Color::Rgb(255 - (h % 255) as u8, 255, 0);
        } else if h < 180 {
            color = Color::Rgb(0, 255, (h % 255) as u8);
        } else if h < 240 {
            color = Color::Rgb(0, 255 - (h % 255) as u8, 255);
        } else if h < 300 {
            color = Color::Rgb((h % 255) as u8, 0, 255);
        } else if h < 360 {
            color = Color::Rgb(255, 0, 255 - (h % 255) as u8);
        }

        datasets.push(
            Dataset::default()
                .name(format!("cpu{}", i))
                .marker(symbols::Marker::Braille)
                .style(Style::default().fg(color))
                .graph_type(GraphType::Line)
                .data(&core_values[i]),
        );
    }

    let v = data
        .iter()
        .enumerate()
        .map(|(i, &x)| ((i as f64), x))
        .collect::<Vec<_>>();
    datasets.push(
        Dataset::default()
            .name("cpu")
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::White))
            .graph_type(GraphType::Line)
            .data(&v),
    );

    let chart = Chart::new(datasets)
        .block(
            Block::default()
                .title(Span::styled(
                    "CPU",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL),
        )
        .x_axis(Axis::default().bounds([0.0, 300.0]))
        .y_axis(
            Axis::default()
                .style(Style::default().fg(Color::Gray))
                .labels(vec![
                    Span::styled("  0", Style::default().add_modifier(Modifier::BOLD)),
                    Span::styled("100", Style::default().add_modifier(Modifier::BOLD)),
                ])
                .bounds([0.0, 100.0]),
        );

    f.render_widget(chart, rect);
}

fn draw_diskinfo<B: Backend>(f: &mut Frame<B>, boxes: &Vec<Rect>, disk_info: &Vec<DiskInfo>) {
    //draw disk info TODO: divide into own function
    let block = Block::default()
        .title(Span::styled(
        "Disks",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
        )).borders(Borders::ALL);
    let header_cells = ["Partition", "Available", "In Use", "Total", "Used", "Mount"]
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

fn draw_processesinfo<B: Backend>(f: &mut Frame<B>, rect: Rect, pl: &ProcessList) {
    let selected_style = Style::default().add_modifier(Modifier::REVERSED);
    let header_style = Style::default().bg(Color::DarkGray).fg(Color::Black);
    let header_cells = [
        "PID", "PPID", "TID", "User", "Umask", "Threads", "Name", "State", "VM", "SM", "CMD",
    ]
        .iter()
        .map(|h| Cell::from(*h));
    let header = Row::new(header_cells)
        .style(header_style)
        .height(1);
    let rows = pl.processes.iter().map(|p| {
        let mut cells = Vec::new();
        cells.push(Cell::from(p.pid.to_string()));
        cells.push(Cell::from(p.parent_pid.to_string()));
        cells.push(Cell::from(p.thread_group_id.to_string()));
        cells.push(Cell::from(p.user.to_string()));
        cells.push(Cell::from(p.umask.to_string()));
        cells.push(Cell::from(p.threads.to_string()));
        cells.push(Cell::from(p.name.to_string()));
        cells.push(Cell::from(p.state.to_string()));
        cells.push(Cell::from(to_humanreadable(p.virtual_memory_size * 1000)));
        cells.push(Cell::from(to_humanreadable(p.swapped_memory * 1000)));
        cells.push(Cell::from(p.command.to_string()));
        Row::new(cells).height(1)
    });
    // println!("{}", rows.len());
    let table = Table::new(rows)
        .header(header)
        .highlight_style(selected_style)
        .widths(&[
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Length(15),
            Constraint::Length(6),
            Constraint::Length(7),
            Constraint::Length(30),
            Constraint::Length(15),
            Constraint::Length(9),
            Constraint::Length(9),
            Constraint::Min(1),
        ])
        .block(
            Block::default()
            .title(Span::styled(
                "Processes",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL),
        );
    f.render_widget(table, rect);

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

fn draw_networkinfo<B: Backend>(
    f: &mut Frame<B>,
    rect: Rect,
    last_info: &NetworkInfo,
    current_info: &NetworkInfo,
) {
    let receiving = 0;//to_humanreadable((current_info.rec_bytes - last_info.rec_bytes) * 10) + "/s";
    let sending = 0;//to_humanreadable((current_info.send_bytes - last_info.send_bytes) * 10) + "/s";
    let total_received = 0;//to_humanreadable(current_info.rec_bytes);
    let total_sent = 0;//to_humanreadable(current_info.send_bytes);

    let block = Block::default()
        .title(Span::styled(
        "Network",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
        )).borders(Borders::ALL);

    let text = vec![
        Spans::from(format!("Receiving      {}", receiving)),
        Spans::from(format!("Total Received {}", total_received)),
        Spans::from(format!("Sending        {}", sending)),
        Spans::from(format!("Total Sent     {}", total_sent)),
    ];
    let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: true });
    f.render_widget(paragraph, rect);
}
