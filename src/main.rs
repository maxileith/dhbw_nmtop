use std::io;
use std::{thread, time::Duration};
use termion::{event::Key, raw::IntoRawMode};
use tui::{
    backend::TermionBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, BorderType, Borders, Paragraph},
    Terminal,
};

// Module for reading keyboard events
mod util;
use util::InputHandler;

// Module for reading CPU usage data
mod cpu;
use cpu::CpuWidget;

// Module for reading memory usage data
mod mem;
use mem::MemoryWidget;

// Module for reading disk usage data
mod disk;
use disk::DiskWidget;

// Module for managing processes
mod processes;
use processes::ProcessesWidget;

// Module for reading network usage
mod network;
use network::NetworkWidget;

/// Defines the different application states.
#[derive(PartialEq)]
enum AppState {
    /// During this state users can navigate between widgets
    Navigation,
    /// During this state users can use current selected widget
    Interaction,
}

/// Stores necessary data to handle the application logic.
struct AppLogic {
    /// current application state [AppState]
    state: AppState,
    /// current selected widget
    current_widget: WidgetType,
    /// defines whether selected widget is highlighted
    show_selected_widget: bool,
}

/// Defines the supported widget types. A widget enables an user to to view specific system information like memory
/// usage, processes or network usage.
#[derive(PartialEq)]
enum WidgetType {
    CPU,
    Network,
    Disk,
    Processes,
    Memory,
}

impl WidgetType {
    /// Returns a tuple containing the id and the name of a widget
    fn get_value(&self) -> (usize, &str) {
        match *self {
            WidgetType::Memory => (0, "Memory"),
            WidgetType::Disk => (1, "Partitions"),
            WidgetType::Network => (2, "Network"),
            WidgetType::CPU => (3, "CPU"),
            WidgetType::Processes => (4, "Processes"),
        }
    }

    /// Returns a widget type by the associated id
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

    /// Returns the help text of a widget
    fn get_help_text(&self) -> &str {
        match *self {
            WidgetType::Memory => "",
            WidgetType::Disk => ", up: previous, down: next",
            WidgetType::Network => "",
            WidgetType::CPU => ", SPACE: show/hide all cores",
            WidgetType::Processes => {
                ", s:sort, left/right:  move header, up/down: select process, n: niceness"
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Terminal initialization
    let stdout = io::stdout().into_raw_mode()?;
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Initialize the different widgets
    let mut disk_widget = DiskWidget::new();
    let mut cpu_widget = CpuWidget::new();
    let mut mem_widget = MemoryWidget::new();
    let mut processes_widget = ProcessesWidget::new();
    let mut network_widget = NetworkWidget::new();

    // Initialize app state
    let mut app = AppLogic {
        state: AppState::Interaction,
        current_widget: WidgetType::Processes,
        show_selected_widget: false,
    };

    // Initialize input handler
    let input_handler = InputHandler::new();

    // Define sleep duration for thread
    const SLEEP_DURATION: Duration = Duration::from_millis(100);

    // Define all used widgets
    let data_widgets = vec![
        WidgetType::Memory,
        WidgetType::Disk,
        WidgetType::Network,
        WidgetType::CPU,
        WidgetType::Processes,
    ];

    // Clear terminal - otherwise the screen may contain old data
    terminal.clear()?;

    loop {
        // Update the widgets
        mem_widget.update();
        cpu_widget.update();
        processes_widget.update();
        disk_widget.update();
        network_widget.update();

        // Draw the tui
        terminal.draw(|f| {
            // Define the top level layout
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
            // Split the box at the top in 3
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

                // Determine whether the widget is selected
                let mut selected = id == app.current_widget.get_value().0;
                // Check whether navigation is active
                let navigation = app.state == AppState::Navigation;

                // If application is in interaction state, check whether the user wants to
                // interact with the selected widget. The widget is highlighted if if the user wants to interact with it.
                if !navigation {
                    selected = selected && app.show_selected_widget;
                }

                // Choose draw method based on widget
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

            // Generate help text which is displayed to user
            let mut help_text =
                "ESC: navigation/interaction, v:view/hide selected widget".to_string();

            if app.show_selected_widget && app.state == AppState::Interaction {
                // Append help text of current selected widget
                help_text += app.current_widget.get_help_text();

                // The help text needs to be dynamically appended since the processes widget provides multiple
                // features depending on the internal state of the widget.
                if app.current_widget == WidgetType::Processes {
                    help_text += processes_widget.get_help_text();
                }
            }

            // Draw help text
            let help_paragraph = Paragraph::new(help_text)
                .block(Block::default())
                .alignment(Alignment::Left);
            f.render_widget(help_paragraph, chunks[3]);
        })?;

        // Get new keyboard events 
        let event = input_handler.next();

        if event.is_ok() {
            let input = event.unwrap();

            // Depending on the app state different key bindings are used
            match app.state {
                AppState::Interaction => {
                    // Input is handled by the selected widget
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
                    
                    // Global shortcuts
                    match input {
                        Key::Char('v') => {
                            app.show_selected_widget = !app.show_selected_widget;
                        }
                        // Switch between app states
                        Key::Esc => {
                            app.state = AppState::Navigation;
                        }
                        Key::Ctrl('c') => {
                            terminal.clear()?;
                            break;
                        }
                        _ => {}
                    };
                }

                AppState::Navigation => {
                    match input {
                        // Navigation
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
                        // Switch between app states
                        Key::Esc => {
                            app.state = AppState::Interaction;
                            app.show_selected_widget = true;
                        }
                        // Global exit shortcut
                        Key::Ctrl('c') => {
                            terminal.clear()?;
                            break;
                        }
                        _ => {}
                    };
                }
            }
        }

        // Sleep
        thread::sleep(SLEEP_DURATION);
    }
    Ok(())
}
/// Creates a new empty block which can be populated by a widget.
/// The border style is dynamically modified based on the selection and navigation state.
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
