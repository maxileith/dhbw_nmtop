use std::collections::VecDeque;
use std::fmt;
use std::fs::File;
use std::sync::mpsc;
use std::{io::BufRead, io::BufReader};
use std::{thread, time};
use termion::event::Key;
use tui::{
    backend::Backend,
    layout::Rect,
    style::{Color, Modifier, Style},
    symbols,
    terminal::Frame,
    text::Span,
    widgets::{Axis, Block, Chart, Dataset, GraphType},
};

use crate::util;

/// Represents a cpu result row of the /proc/stat content
///
/// Time units are in USER_HZ or Jiffies
/// See https://www.linuxhowtos.org/System/procstat.htm
#[derive(Clone)]
pub struct ProcStatRow {
    /// Name of the CPU
    pub cpu_name: String,
    /// Normal processes user mode
    pub normal_proc_user_mode: u32,
    /// Niced proccesses user mode
    pub nice_proc_user_mode: u32,
    /// Proccesses kernel mode
    pub system_proc_kernel_mode: u32,
    pub idle: u32,
    /// waiting for I/O
    pub iowait: u32,
    /// servicing interrupts
    pub irq: u32,
    /// servicing softirqs
    pub softirq: u32,
}

impl ProcStatRow {
    /// Calculate total cpu calculation time.
    ///
    /// Adds differnt cpu usage time together.
    ///
    /// # Panic
    ///
    /// This function won't panic.
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

/// Determines whether the CPU name is parsed or the values of a CPU.
#[derive(PartialEq)]
enum ReadingMode {
    CpuName,
    CpuValue,
}

/// Stores the cpu utilization of a specific cpu (core)
pub struct CpuUtilization {
    pub cpu_name: String,
    pub utilization: f64,
}

impl fmt::Display for CpuUtilization {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} uses {}", self.cpu_name, self.utilization)
    }
}

/// Calculates and returns the cpu utilization based on two different measured cpu times.
///
/// # Arguments
///
/// * 'previous' - previous measured cpu time
/// * 'current' - current measured cpu time
///
/// # Panic
///
/// This function won't panic.
fn calculate_cpu_utilization(previous: &ProcStatRow, current: &ProcStatRow) -> f64 {
    let previous_total_elapsed = previous.get_total_time();
    let current_total_elapsed = current.get_total_time();

    let total_delta = (current_total_elapsed - previous_total_elapsed) as f64;
    let idle_delta = (current.idle - previous.idle) as f64;
    let utilization: f64 = 100.0 * (1.0 - idle_delta / total_delta);
    utilization
}

/// Opens and returns a new file handle to the /proc/stat file.
///
/// # Panic
///
/// This function won't panic.
fn get_proc_stat_file_handle() -> Option<File> {
    let file_handle = File::open("/proc/stat");

    let file = match file_handle {
        Ok(x) => Some(x),
        Err(_) => None,
    };
    file
}


/// Reads and parses the cpu utilization provided by /proc/stat
///
/// # Arguments
///
/// * 'stats' - queue of cpu stats which should be temporarly saved
/// * 'iteration_count' - current measured cpu time
///
/// # Panic
///
/// This function won't panic.
fn update_current_cpu_utilization(
    stats: &mut VecDeque<ProcStatRow>,
    iteration_count: &u32,
) -> Vec<CpuUtilization> {
    let mut result = Vec::<CpuUtilization>::new();

    // Open file handle and read file if successful
    // Otherwise return empty vec.
    if let Some(file) = get_proc_stat_file_handle() {
        let reader = BufReader::new(file);

        let mut reading_mode;

        for line in reader.lines() {
            let row = match line {
                Ok(x) => x,
                Err(_) => break,
            };

            // We only care about cpu information, so discard other lines
            if row.starts_with("cpu") {
                reading_mode = ReadingMode::CpuName;

                let mut current_cpu_name: &str = "";
                let mut values: [u32; 10] = [0; 10];
                let mut field_counter = 0;

                for z in row.split_whitespace() {
                    match reading_mode {
                        // Read cpu name from line
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
                    cpu_name: current_cpu_name.to_string(),
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

                    let utilization = CpuUtilization {
                        cpu_name: current_cpu_name.to_string(),
                        utilization: calculate_cpu_utilization(&previous_stat, &current_stat),
                    };
                    result.push(utilization);
                }
                stats.push_back(current_stat);
            }
        }
    }
    result
}

/// Initializes a thread to collect and send the process list each 0.5 seconds.
///
/// Calculates current cpu utilization and sends the result to the receiver
/// which was originally returned by the function.
///
/// # Panic
///
/// This function won't panic.
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
    show_all_cores: bool,
    dc_thread: mpsc::Receiver<Vec<CpuUtilization>>,
}

impl CpuWidget {
    /// Returns a new CpuWidget with default values and a new data thread.
    ///
    /// # Panic
    ///
    /// This function won't panic.
    pub fn new() -> Self {
        Self {
            core_values: Vec::<Vec<f64>>::new(),
            cpu_values: Vec::<f64>::new(),
            show_all_cores: true,
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

    /// Draws cpu utilization graph in a given Rect.
    ///
    /// Each cpu cores is rendered in a different color.
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
    pub fn draw<B: Backend>(&self, f: &mut Frame<B>, rect: Rect, block: Block) {
        let mut datasets = Vec::new();

        let mut values = Vec::new(); //FIXME: ugly should fix

        if self.show_all_cores {
            for core in &self.core_values {
                let value = core
                    .iter()
                    .enumerate()
                    .map(|(i, &x)| ((i as f64), x))
                    .collect::<Vec<_>>();
                values.push(value);
            }

            for i in 0..values.len() {
                let color = util::get_color_by_scalar(i);

                datasets.push(
                    Dataset::default()
                        .name(format!("cpu{}", i))
                        .marker(symbols::Marker::Braille)
                        .style(Style::default().fg(color))
                        .graph_type(GraphType::Line)
                        .data(&values[i]),
                );
            }
        }

        let v = self
            .cpu_values
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

        // Create new chart with datasets
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
        match key {
            // Show or hide all cores in chart
            Key::Char(' ') => self.show_all_cores = !self.show_all_cores,
            _ => {}
        };
    }
}
