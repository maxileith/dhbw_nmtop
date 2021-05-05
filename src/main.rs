use std::io;
use std::vec::Vec;
use std::{thread, time};
use termion::{event::Key, raw::IntoRawMode};
use tui::{
    backend::{Backend, TermionBackend},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    terminal::Frame,
    text::{Span, Spans},
    widgets::{
        Axis, Block, BorderType, Borders, Cell, Chart, Dataset, Gauge, GraphType, Paragraph, Row,
        Table, Wrap,
    },
    Terminal,
};

mod util;

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

#[derive(PartialEq)]
enum AppState {
    Navigation,
    Interaction,
}

struct AppLogic {
    state: AppState,
    current_widget: WidgetType,
    show_selected_widget: bool,
    show_all_cores: bool,
}

#[derive(PartialEq)]
enum WidgetType {
    CPU,
    Network,
    Disk,
    Processes,
    Memory,
}

impl WidgetType {
    // returns id, name
    fn get_value(&self) -> (usize, &str) {
        match *self {
            WidgetType::Memory => (0, "Memory"),
            WidgetType::Disk => (1, "Disk"),
            WidgetType::Network => (2, "Network"),
            WidgetType::CPU => (3, "CPU"),
            WidgetType::Processes => (4, "Processes"),
        }
    }

    fn get_by_id(id: usize) -> WidgetType {
        match id {
            0 => WidgetType::Memory,
            1 => WidgetType::Disk,
            2 => WidgetType::Network,
            3 => WidgetType::CPU,
            4 => WidgetType::Processes,
            _ => WidgetType::Memory, //default case
        }
    }

    fn get_help_text(&self) -> &str {
        match *self {
            WidgetType::Memory => "",
            WidgetType::Disk => "up: previous page, down: next page",
            WidgetType::Network => "",
            WidgetType::CPU => "SPACE: show/hide all cores",
            WidgetType::Processes => {
                "s:sort, left/right:  move header, up/down: select process"
            }
        }
    }
}

struct DataWidget {
    typ: WidgetType,
}

// TODO: user input to stop execution
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Terminal initialization
    let stdout = io::stdout().into_raw_mode()?;
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut disk_widget = disk::DiskWidget::new();
    let mut cpu_widget = cpu::CpuWidget::new();
    
    // Initialize app state
    let mut app = AppLogic {
        state: AppState::Interaction,
        current_widget: WidgetType::Memory,
        show_selected_widget: false,
        show_all_cores: true,
    };

    // Initialize input handler
    let input_handler = util::InputHandler::new();

    let mem_dc_thread = mem::init_data_collection_thread();
    let processes_dc_thread = processes::init_data_collection_thread();
    let network_dc_thread = network::init_data_collection_thread();

    let sleep_duration = time::Duration::from_millis(100);

    let mut last_network_info: NetworkInfo = Default::default();

    let mut processes_info: ProcessList = Default::default();
    let mut mem_info: MemInfo = Default::default();
    //let mut disk_info: std::vec::Vec<disk::DiskInfo> = Default::default();
    let mut network_info: NetworkInfo = Default::default();

    let data_widgets = vec![
        WidgetType::Memory,
        WidgetType::Disk,
        WidgetType::Network,
        WidgetType::CPU,
        WidgetType::Processes,
    ];

    //let mut cpu_values = Vec::<f64>::new();
    terminal.clear()?;
    loop {
        mem_info = match mem_dc_thread.try_recv() {
            Ok(a) => a,
            Err(_) => mem_info,
        };

        cpu_widget.update();
    
        disk_widget.update();

        // Recv data from the data collector thread
        processes_info = match processes_dc_thread.try_recv() {
            Ok(a) => a,
            Err(_) => processes_info,
        };

        network_info = match network_dc_thread.try_recv() {
            Ok(a) => {
                last_network_info = network_info;
                a
            }
            Err(_) => network_info,
        };

        let _ = terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints(
                    [
                        Constraint::Length(6),
                        Constraint::Length(10),
                        Constraint::Min(1),
                        Constraint::Length(1),
                    ]
                    .as_ref(),
                )
                .split(f.size());
            let boxes = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(
                    [
                        Constraint::Percentage(30),
                        Constraint::Percentage(45),
                        Constraint::Percentage(25),
                    ]
                    .as_ref(),
                )
                .split(chunks[0]);

            // Draw data widgets
            for dw in &data_widgets {
                let (id, name) = dw.get_value();

                let mut selected = id == app.current_widget.get_value().0;
                let navigation = app.state == AppState::Navigation;

                if !navigation {
                    selected = selected && app.show_selected_widget; 
                }

                match dw {
                    WidgetType::Memory => {
                        draw_meminfo(f, boxes[0], create_block(name, selected, navigation), &mem_info);
                    }
                    WidgetType::Disk => {
                        disk_widget.draw(f, boxes[1], create_block(name, selected, navigation));
                        //draw_diskinfo(f, boxes[1], create_block(name, selected, navigation), &disk_info);
                    }
                    WidgetType::Network => {
                        draw_networkinfo(
                            f,
                            boxes[2],
                            create_block(name, selected, navigation),
                            &last_network_info,
                            &network_info,
                        );
                    }
                    WidgetType::CPU => {
                        cpu_widget.draw(
                            f,
                            chunks[1],
                            create_block(name, selected, navigation),
                        );
                    }
                    WidgetType::Processes => {
                        draw_processesinfo(
                            f,
                            chunks[2],
                            create_block(name, selected, navigation),
                            &processes_info,
                        );
                    }
                }
            }

            let mut help_text = "ESC: navigation/interaction, v:view/hide selected widget, ".to_string();

            if app.show_selected_widget{
                help_text += app.current_widget.get_help_text(); // TODO: make constant
            }

            // Draw help text
            let help_paragraph = Paragraph::new(help_text)
                .block(Block::default())
                .alignment(Alignment::Left);
            f.render_widget(help_paragraph, chunks[3]);
        });

        // Handle events
        let event = input_handler.next();

        if event.is_ok() {
            let input = event.unwrap();
            match app.state {
                AppState::Interaction => {

                    if app.show_selected_widget {
                        match app.current_widget {
                            WidgetType::Processes => {
                                handle_processes_input(input, &mut app);
                            }, 
                            WidgetType::CPU => {
                                cpu_widget.handle_input(input);
                            },
                            WidgetType::Disk => {
                                disk_widget.handle_input(input);
                            },
                            _ => {},
                        }
                    }

                    match input {
                        Key::Ctrl('c') => {
                            terminal.clear()?;
                            break;
                        }
                        Key::Char('v') => {
                            app.show_selected_widget = !app.show_selected_widget;
                        }
                        Key::Esc => {
                            app.state = AppState::Navigation;
                        }
                        _ => {}
                    };
                }

                AppState::Navigation => {
                    match input {
                        Key::Ctrl('c') => {
                            terminal.clear()?;
                            break;
                        }
                        Key::Right => {
                            let (id, _) = app.current_widget.get_value();
                            if id < data_widgets.len() - 1 {
                                app.current_widget = WidgetType::get_by_id(id + 1);
                            }
                        }
                        Key::Left => {
                            let (id, _) = app.current_widget.get_value();
                            if id > 0 {
                                app.current_widget = WidgetType::get_by_id(id - 1);
                            }
                        }
                        Key::Esc => {
                            app.state = AppState::Interaction;
                            app.show_selected_widget = true;
                        }
                        _ => {}
                    };
                }
            }
        }

        // Sleep
        thread::sleep(sleep_duration);
    }
    Ok(())
}

fn handle_processes_input(key: Key, app: &mut AppLogic) {
    match key {
        Key::Right => {
            println!("right pressed");
        }
        Key::Left => {
        }
        Key::Up=> {
        }
        Key::Down=> {
        }
        _ => {}
    };
}

fn create_block(name: &str, selected: bool, navigation: bool) -> Block {
    let mut color = Color::Cyan;
    
    if !navigation && selected {
       color = Color::Yellow; 
    }
    

    let block = Block::default()
        .title(Span::styled(
            name,
            Style::default()
                .fg(color)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL);
    

    if selected {
        return block.border_type(BorderType::Thick);
    }

    block
}

fn draw_meminfo<B: Backend>(f: &mut Frame<B>, rect: Rect, block: Block, mem_info: &MemInfo) {
    let block_chunks = Layout::default()
        .constraints([Constraint::Length(2), Constraint::Length(2)])
        .margin(1)
        .split(rect);

    // Render block
    f.render_widget(block, rect);

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



fn draw_processesinfo<B: Backend>(f: &mut Frame<B>, rect: Rect, block: Block, pl: &ProcessList) {
    let selected_style = Style::default().add_modifier(Modifier::REVERSED);
    let header_style = Style::default().bg(Color::DarkGray).fg(Color::White);
    let header_cells = [
        "PID", "PPID", "TID", "User", "Umask", "Threads", "Name", "State", "VM", "SM", "CMD",
    ]
    .iter()
    .map(|h| Cell::from(*h));
    let header = Row::new(header_cells).style(header_style).height(1);

    // add code to filter process list

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
        .block(block);
    f.render_widget(table, rect);
}

fn draw_networkinfo<B: Backend>(
    f: &mut Frame<B>,
    rect: Rect,
    block: Block,
    last_info: &NetworkInfo,
    current_info: &NetworkInfo,
) {
    if last_info.rec_bytes > current_info.rec_bytes {
        return;
    }

    let receiving = to_humanreadable((current_info.rec_bytes - last_info.rec_bytes) * 2) + "/s";
    let sending = to_humanreadable((current_info.send_bytes - last_info.send_bytes) * 2) + "/s";
    let total_received = to_humanreadable(current_info.rec_bytes);
    let total_sent = to_humanreadable(current_info.send_bytes);

    let text = vec![
        Spans::from(format!("Receiving      {}", receiving)),
        Spans::from(format!("Total Received {}", total_received)),
        Spans::from(format!("Sending        {}", sending)),
        Spans::from(format!("Total Sent     {}", total_sent)),
    ];
    let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: true });
    f.render_widget(paragraph, rect);
}
