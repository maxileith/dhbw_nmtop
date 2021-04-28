use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader};
use termion::raw::IntoRawMode;
use tui::backend::TermionBackend;
use tui::layout::{Constraint, Direction, Layout};
use tui::style::{Color, Modifier, Style};
use tui::widgets::{Block, Borders, Gauge, Widget};
use tui::Terminal;
use std::{thread, time};
//use std::vec::Vec;
use std::collections::VecDeque;

/// Represents a result row of the /proc/stat content
/// Time units are in USER_HZ or Jiffies
#[derive(Clone)]
struct ProcStatRow {
    pub cpu_name: String,
    pub normal_proc_user_mode: u32,
    pub nice_proc_user_mode: u32,
    pub system_proc_kernel_mode: u32,
    pub idle: u32,
    pub iowait: u32,  // waiting for I/O
    pub irq: u32,     // servicing interupts
    pub softirq: u32, // servicing softirqs
}

impl ProcStatRow {
    fn get_total_time(&self) -> u32 {
        self.normal_proc_user_mode
            + self.nice_proc_user_mode
            + self.system_proc_kernel_mode
            + self.idle
            + self.iowait
            + self.irq
            + self.softirq
    }
}

#[derive(PartialEq)]
enum ReadingMode {
    CpuName,
    CpuValue,
}

#[derive(Default, Debug)]
struct MemInfo {
    pub mem_total: f64,
    pub mem_free: f64,
    pub mem_available: f64,
    pub swap_total: f64,
    pub swap_free: f64,
    pub swap_cached: f64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proc_stat = "/proc/stat";
    /*
    let mut previous_stat = ProcStatRow {
        cpu_name: "cpu".to_string(), // ugly, should find better way
        softirq: 0,
        irq: 0,
        iowait: 0,
        idle: 0,
        system_proc_kernel_mode: 0,
        nice_proc_user_mode: 0,
        normal_proc_user_mode: 0,
    };*/
    let mut stats: VecDeque<ProcStatRow> = VecDeque::new(); // create with fixed size
    let mut iteration_count = 0;
    let stdout = io::stdout().into_raw_mode()?;
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut mem_info: MemInfo = Default::default();
    let sleep_duration = time::Duration::from_millis(100);

    terminal.clear()?;
    loop {
        let file = File::open(proc_stat)?;
        let reader = BufReader::new(file);

        let mut reading_mode;

        for line in reader.lines() {
            let row = line?;
            if row.starts_with("cpu") {
                reading_mode = ReadingMode::CpuName;

                let mut current_cpu_name: &str = "";
                let mut values: [u32; 10] = [0; 10];
                let mut field_counter = 0;

                for z in row.split_whitespace() {
                    match reading_mode {
                        ReadingMode::CpuName => {
                            current_cpu_name = z;
                            reading_mode = ReadingMode::CpuValue;
                        }

                        ReadingMode::CpuValue => {
                            let number: u32 = match z.trim().parse() {
                                Err(_) => 0,
                                Ok(n) => n,
                            };

                            values[field_counter] = number;
                            field_counter += 1;
                        }
                    }
                }

                let current_stat = ProcStatRow {
                    cpu_name: current_cpu_name.to_string(), // ugly, should find better way
                    softirq: values[6],
                    irq: values[5],
                    iowait: values[4],
                    idle: values[3],
                    system_proc_kernel_mode: values[2],
                    nice_proc_user_mode: values[1],
                    normal_proc_user_mode: values[0],
                };
            
                
                if iteration_count > 0 {
                    //println!("{}", current_stat.cpu_name);
                    let previous_stat = match stats.pop_front() {
                        Some(x) => x,
                        None => {
                            break;
                        },
                    };
                    //println!("{}", previous_stat.cpu_name);
                    /*println!(
                        "{} Utilization {}%",
                        current_cpu_name,
                        calculate_cpu_utilization(&previous_stat, &current_stat)
                    );*/
                }
                stats.push_back(current_stat);
            }
        }

        let _ = show_ram_usage(&mut mem_info);

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
            .constraints(
                [
                    Constraint::Percentage(50),
                    Constraint::Percentage(50),
                ]
                .as_ref(),
            )
            .split(chunks[0]);
            let block_chunks = Layout::default()
                .constraints([Constraint::Length(2), Constraint::Length(2)])
                .margin(1)
                .split(boxes[0]);

            let block = Block::default().title("Mem").borders(Borders::ALL);
            f.render_widget(block, boxes[0]);
            let block1 = Block::default().title("Block 2").borders(Borders::ALL);
            f.render_widget(block1, chunks[1]);
            let block2 = Block::default().title("Block2").borders(Borders::ALL);
            f.render_widget(block2, chunks[2]);
            // calc mem infos
            let mem_usage = (mem_info.mem_total - mem_info.mem_available) / mem_info.mem_total;
            let mem_swap = mem_info.swap_cached / mem_info.swap_total;
            let label_mem = format!("{:.2}%", mem_usage * 100.0);
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
            let label_swap = format!("{:.2}%", mem_swap * 100.0);
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
        });

        let dur = time::Duration::from_millis(1000);
        thread::sleep(dur);

        iteration_count += 1;
    }

   //Ok(())
}

fn calculate_cpu_utilization(previous: &ProcStatRow, current: &ProcStatRow) -> f32 {
    let previous_total_elapsed = previous.get_total_time();
    let current_total_elapsed = current.get_total_time();

    let total_delta = (current_total_elapsed - previous_total_elapsed) as f32;
    let idle_delta = (current.idle - previous.idle) as f32;
    let utilization: f32 = 100.0 * (1.0 - idle_delta / total_delta);
    utilization
}

fn show_ram_usage(mem_info: &mut MemInfo) -> Result<(), Box<dyn std::error::Error>> {
    let meminfo = "/proc/meminfo";

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

    //println!("{:?}", mem_info);

    Ok(())
}
