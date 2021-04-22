use std::fs::File;
use std::io::{BufRead, BufReader};
//use std::vec::Vec;
use std::collections::VecDeque;
use std::{thread, time};

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
                    println!(
                        "{} Utilization {}%",
                        current_cpu_name,
                        calculate_cpu_utilization(&previous_stat, &current_stat)
                    );
                }
                stats.push_back(current_stat);
            }
        }
        show_ram_usage();
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

fn show_ram_usage() -> Result<(), Box<dyn std::error::Error>> {
    let meminfo = "/proc/meminfo";

    let file = File::open(meminfo)?;
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let row = line?;
        
        if row.starts_with("Mem") || row.starts_with("Swap") {
            let mut row_values = row.split_whitespace();
            match row_values.next() {
                Some(x) => print!("{}\t", x),
                None => break
            }
            match row_values.next() {
                Some(x) => println!("{:>8} kB", x),
                None => break
            }
        }
    }

    Ok(())
}
