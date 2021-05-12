use std::io;
use std::{thread, time};
use termion::{event::Key, raw::IntoRawMode};
use tui::{
    backend::TermionBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, BorderType, Borders, Paragraph},
    Terminal,
};

mod util;

// Module for reading CPU usage data
mod cpu;

// Module for reading memory usage data
mod mem;

// Module for reading disk usage data
mod disk;

// Module for managing processes
mod processes;

// Module for reading network usage
mod network;

#[derive(PartialEq)]
enum AppState {
    Navigation,
    Interaction,
}

struct AppLogic {
    state: AppState,
    current_widget: WidgetType,
    show_selected_widget: bool,
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
            WidgetType::Disk => ", up: previous, down: next",
            WidgetType::Network => "",
            WidgetType::CPU => ", SPACE: show/hide all cores",
            WidgetType::Processes => ", s:sort, left/right:  move header, up/down: select process, n: niceness, f: filter",
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
    let mut mem_widget = mem::MemoryWidget::new();
    let mut processes_widget = processes::ProcessesWidget::new();
    let mut network_widget = network::NetworkWidget::new();

    // Initialize app state
    let mut app = AppLogic {
        state: AppState::Interaction,
        current_widget: WidgetType::Processes,
        show_selected_widget: false,
    };

    // Initialize input handler
    let input_handler = util::InputHandler::new();

    let sleep_duration = time::Duration::from_millis(100);

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
        mem_widget.update();
        cpu_widget.update();
        processes_widget.update();
        disk_widget.update();
        network_widget.update();

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
                        mem_widget.draw(f, boxes[0], create_block(name, selected, navigation));
                    }
                    WidgetType::Disk => {
                        disk_widget.draw(f, boxes[1], create_block(name, selected, navigation));
                    }
                    WidgetType::Network => {
                        network_widget.draw(f, boxes[2], create_block(name, selected, navigation));
                    }
                    WidgetType::CPU => {
                        cpu_widget.draw(f, chunks[1], create_block(name, selected, navigation));
                    }
                    WidgetType::Processes => {
                        processes_widget.draw(
                            f,
                            chunks[2],
                            create_block(name, selected, navigation),
                        );
                    }
                }
            }

            let mut help_text =
                "ESC: navigation/interaction, v:view/hide selected widget".to_string();

            if app.show_selected_widget && app.state == AppState::Interaction {
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
                                processes_widget.handle_input(input);
                            }
                            WidgetType::CPU => {
                                cpu_widget.handle_input(input);
                            }
                            WidgetType::Disk => {
                                disk_widget.handle_input(input);
                            }
                            WidgetType::Network => {
                                network_widget.handle_input(input);
                            }
                            WidgetType::Memory => {
                                mem_widget.handle_input(input);
                            }
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
                        Key::Up => {
                            let (id, _) = app.current_widget.get_value();
                            if id == 3 {
                                app.current_widget = WidgetType::get_by_id(1);
                            } else if id == 4 {
                                app.current_widget = WidgetType::get_by_id(3);
                            }
                        }
                        Key::Down => {
                            let (id, _) = app.current_widget.get_value();
                            if id < 3 {
                                app.current_widget = WidgetType::get_by_id(3);
                            } else if id == 3 {
                                app.current_widget = WidgetType::get_by_id(4);
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

fn create_block(name: &str, selected: bool, navigation: bool) -> Block {
    let mut color = Color::Cyan;

    if !navigation && selected {
        color = Color::Yellow;
    }

    let block = Block::default()
        .title(Span::styled(
            name,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL);

    if selected {
        return block.border_type(BorderType::Thick);
    }

    block
}
