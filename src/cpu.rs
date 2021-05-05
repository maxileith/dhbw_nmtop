use std::collections::VecDeque;
use std::fmt;
use std::fs::File;
use std::sync::mpsc;
use std::{io::BufRead, io::BufReader};
use std::{thread, time};
use termion::{event::Key};
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

/// Represents a result row of the /proc/stat content
/// Time units are in USER_HZ or Jiffies
#[derive(Clone)]
pub struct ProcStatRow {
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

pub struct CpuUtilization {
    pub cpu_name: String,
    pub utilization: f64,
}

impl fmt::Display for CpuUtilization {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} uses {}", self.cpu_name, self.utilization)
    }
}

fn calculate_cpu_utilization(previous: &ProcStatRow, current: &ProcStatRow) -> f64 {
    let previous_total_elapsed = previous.get_total_time();
    let current_total_elapsed = current.get_total_time();

    let total_delta = (current_total_elapsed - previous_total_elapsed) as f64;
    let idle_delta = (current.idle - previous.idle) as f64;
    let utilization: f64 = 100.0 * (1.0 - idle_delta / total_delta);
    utilization
}

fn update_current_cpu_utilization(
    stats: &mut VecDeque<ProcStatRow>,
    iteration_count: &u32,
) -> Vec<CpuUtilization> {
    let file_handle = File::open("/proc/stat");

    let file = match file_handle {
        Ok(x) => x,
        Err(_) => panic!("Couldn't read stat file"),
    };

    let reader = BufReader::new(file);

    let mut reading_mode;

    let mut result = Vec::<CpuUtilization>::new();

    for line in reader.lines() {
        let row = match line {
            Ok(x) => x,
            Err(_) => break,
        };

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
            if *iteration_count > 0 {
                //println!("{}", current_stat.cpu_name);
                let previous_stat = match stats.pop_front() {
                    Some(x) => x,
                    None => {
                        break;
                    }
                };
                //println!("{}", previous_stat.cpu_name);
                /*println!(
                    "{} Utilization {}%",
                    current_cpu_name,
                    calculate_cpu_utilization(&previous_stat, &current_stat)
                );*/
                let utilization = CpuUtilization {
                    cpu_name: current_cpu_name.to_string(),
                    utilization: calculate_cpu_utilization(&previous_stat, &current_stat),
                };
                result.push(utilization);
            }
            stats.push_back(current_stat);
        }
    }
    result
}

pub fn init_data_collection_thread() -> mpsc::Receiver<Vec<CpuUtilization>> {
    let (tx, rx) = mpsc::channel();

    let mut stats: VecDeque<ProcStatRow> = VecDeque::new(); // create with fixed size
    let mut iteration_count = 0;

    let dur = time::Duration::from_millis(500);

    // Thread for the data collection
    thread::spawn(move || loop {
        let result = update_current_cpu_utilization(&mut stats, &iteration_count);

        let _ = tx.send(result);

        thread::sleep(dur);

        iteration_count += 1;
    });

    rx
}

pub struct CpuWidget {
    core_values: std::vec::Vec<Vec<f64>>,
    cpu_values: std::vec::Vec<f64>,
    dc_thread: mpsc::Receiver<Vec<CpuUtilization>>,
}


// TODO: simplify code and refactor
impl CpuWidget {
    pub fn new() -> Self {
        Self {
            core_values: Vec::<Vec<f64>>::new(),
            cpu_values: Vec::<f64>::new(),
            dc_thread: init_data_collection_thread(),
        }
    }

    pub fn update(&mut self) {
        // Recv data from the data collector thread
        let cpu_stats = match self.dc_thread.try_recv() {
            Ok(a) => a,
            Err(_) => vec![],
        };

        // create cpu info
        let mut counter = 0;
        for b in cpu_stats {
            if b.cpu_name == "cpu" {
                if self.cpu_values.len() == 300 {
                    self.cpu_values.remove(0);
                }
                self.cpu_values.push(b.utilization);
            } else {
                if self.core_values.len() > counter {
                    if self.core_values[counter].len() == 300 {
                        self.core_values[counter].remove(0);
                    }
                    self.core_values[counter].push(b.utilization);
                } else {
                    self.core_values.push(Vec::new());
                    self.core_values[counter].push(b.utilization);
                }
                counter += 1
            }
        }
    }

    pub fn draw<B: Backend>(&self, f: &mut Frame<B>, rect: Rect, block: Block) {
        let mut datasets = Vec::new();

        let mut values = Vec::new(); //FIXME: ugly should fix
        for core in &self.core_values {
            let value = core
                .iter()
                .enumerate()
                .map(|(i, &x)| ((i as f64), x))
                .collect::<Vec<_>>();
            values.push(value);
        }

        for i in 0..values.len() {
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
                    .data(&values[i]),
            );
        }

        let v = self.cpu_values
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
            .block(block)
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

    pub fn handle_input(&mut self, key: Key) {
        /*match key {
            Key::Char(' ') => app.show_all_cores = !app.show_all_cores,
            _ => {},
        };*/
    }
}
