use std::fs::File;
use std::io::{BufRead, BufReader};
use std::io;

/// Represents a result row of the /proc/stat content
/// Time units are in USER_HZ or Jiffies
struct ProcStatRow {
    normal_proceses_user_mode: u32,
    nice_processes_user_mode: u32,
    system_process_kernel_mode: u32,
    idle: u32,
    iowait: u32, // waiting for I/O
    irq: u32, // servicing interupts
    softirq: u32 // servicing softirqs 
}

fn main() -> io::Result<()>{
    let proc_stat = "/proc/stat";
    println!("Reading from {}", proc_stat);
    
    let file = File::open(proc_stat)?;
    let reader = BufReader::new(file);
    let mut keyword_active = false;
    let mut current_proc_stat;

    for line in reader.lines() {
        for i in line?.split_ascii_whitespace() {
            if i.contains("cpu") {
                keyword_active != keyword_active;
            }
            
            if keyword_active {
            }
        }
    }

    Ok(())
}
