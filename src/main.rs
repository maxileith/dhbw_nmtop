use std::io;
use std::vec::Vec;
use std::{thread, time};
use termion::{event::Key, raw::IntoRawMode};
use tui::{
    backend::{Backend, TermionBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    terminal::Frame,
    text::Span,
    symbols,
    widgets::{Axis, Block, Borders, Chart, Gauge, Dataset, GraphType},
    Terminal,
};

mod util;
use util::InputEvent;

// Module for reading CPU usage data
mod cpu;

// Module for reading memory usage data
mod mem;
use mem::MemInfo;

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

    let sleep_duration = time::Duration::from_millis(100);

    let mut core_values = Vec::<Vec<f64>>::new();
    let mut cpu_values = Vec::<f64>::new();

    //let mut cpu_values = Vec::<f64>::new();
    terminal.clear()?;
    
    loop {
        let mem_info = match mem_dc_thread.try_recv() {
            Ok(a) => a,
            Err(_) => Default::default(),
        };

        // Recv data from the data collector thread
        let cpu_stats = match cpu_dc_thread.try_recv() {
            Ok(a) => a,
            Err(_) => vec![],
        };
        // create cpu info
        let mut counter = 0;
        for b in cpu_stats {
            if b.cpu_name == "cpu"{
                if cpu_values.len() == 300{
                   cpu_values.remove(0);
                } 
                cpu_values.push(b.utilization);
            }else {
                if core_values.len() > counter {
                    if core_values[counter].len() == 300{
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

        terminal.draw(|f| {
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
            let block1 = Block::default().title("Block 2").borders(Borders::ALL);
            f.render_widget(block1, chunks[1]);
            let block2 = Block::default().title("Block2").borders(Borders::ALL);
            f.render_widget(block2, chunks[2]);

            draw_meminfo(f, chunks[0], &mem_info);
            draw_cpuinfo(f, chunks[1], &cpu_values, &core_values);
        })?;

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

fn draw_meminfo<B: Backend>(f: &mut Frame<B>, rect: Rect, mem_info: &MemInfo) {
    let boxes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(rect);
    let block_chunks = Layout::default()
        .constraints([Constraint::Length(2), Constraint::Length(2)])
        .margin(1)
        .split(boxes[0]);

    let block = Block::default().title("Mem").borders(Borders::ALL);
    f.render_widget(block, boxes[0]);

    // calc mem infos
    let mem_usage = (mem_info.mem_total - mem_info.mem_available) / mem_info.mem_total;
    let mem_swap = mem_info.swap_cached / mem_info.swap_total;
    let label_mem = format!("{:.2}%", mem_usage * 100.0);

    if 0.0 <= mem_usage && mem_usage <= 1.0 {
        let gauge_mem = Gauge::default()
            .block(Block::default().title("Mem:"))
            .gauge_style(
                Style::default()
                    .fg(Color::Magenta)
                    .bg(Color::Black)
                    .add_modifier(Modifier::ITALIC | Modifier::BOLD),
            )
            .label(label_mem)
            .ratio(mem_usage);
        f.render_widget(gauge_mem, block_chunks[0]);
    }

    let label_swap = format!("{:.2}%", mem_swap * 100.0);

    if 0.0 <= mem_swap && mem_swap <= 1.0 {
        let gauge_swap = Gauge::default()
            .block(Block::default().title("Swap:"))
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
}


fn draw_cpuinfo<B: Backend>(f: &mut Frame<B>, rect: Rect, data: &Vec<f64>, cores: &Vec<Vec<f64>>) {
    let mut datasets = Vec::new();

    let mut core_values = Vec::new();
    for core in cores {
        let value = core.iter().enumerate().map(|(i, &x)| ((i as f64), x)).collect::<Vec<_>>();
        core_values.push(value);
    }
    let l = core_values.len();

    for i in 0..l {
        let f = i as f64 /l as f64;
        let r:u8 = (f * 255.0).round() as u8;
        let g:u8 = (f * 255.0).round() as u8;
        let b:u8 = (f * 255.0).round() as u8;

        datasets.push(Dataset::default()
            .name(format!("cpu{}", i))
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::Rgb(r,g,b)))
            .graph_type(GraphType::Line)
            .data(&core_values[i]));
    }
    
    let v = data.iter().enumerate().map(|(i, &x)| ((i as f64), x)).collect::<Vec<_>>();
    datasets.push(Dataset::default()
            .name("cpu")
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::Cyan))
            .graph_type(GraphType::Line)
            .data(&v));

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
                    Span::styled("0", Style::default().add_modifier(Modifier::BOLD)),
                    Span::styled("100", Style::default().add_modifier(Modifier::BOLD)),
                ])
                .bounds([0.0, 100.0]),
        );

    f.render_widget(chart, rect);
}
