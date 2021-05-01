use std::collections::VecDeque;
use std::fmt;
use std::fs::File;
use std::{io::BufRead, io::BufReader};
use std::{thread, time};
use std::sync::mpsc;

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
    
    let dur = time::Duration::from_millis(150);

    // Thread for the data collection
    let proc_stat_thread = thread::spawn(move || loop {
        let result = update_current_cpu_utilization(&mut stats, &iteration_count);
        
        tx.send(result);

        thread::sleep(dur);

        iteration_count += 1;
    });

    rx
}

