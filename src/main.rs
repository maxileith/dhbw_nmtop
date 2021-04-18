use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader};
use std::vec::Vec;

/// Represents a result row of the /proc/stat content
/// Time units are in USER_HZ or Jiffies
struct ProcStatRow {
    cpu_name: String,
    normal_proc_user_mode: u32,
    nice_proc_user_mode: u32,
    system_proc_kernel_mode: u32,
    idle: u32,
    iowait: u32,  // waiting for I/O
    irq: u32,     // servicing interupts
    softirq: u32, // servicing softirqs
}


enum ReadingMode {
    CPU_NAME,
    CPU_VALUE,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proc_stat = "/proc/stat";
    println!("Reading from {}", proc_stat);

    let file = File::open(proc_stat)?;
    let reader = BufReader::new(file);
    let mut keyword_active = false;
    let mut current_cpu_name: String;
    let mut values: Vec<u32> = Vec::new();
    let mut stats: Vec<ProcStatRow> = Vec::new();
   
    let mut reading_mode = ReadingMode::CPU_NAME;
    let mut previous_index = 0;    
    let mut found_number = false;

    for line in reader.lines() {
        let row = line?;
        if row.starts_with("cpu") {
            println!("{}", row);
            previous_index = 0;
            reading_mode = ReadingMode::CPU_NAME;
            for (i, c) in row.chars().enumerate() {
                match reading_mode {
                    ReadingMode::CPU_NAME => {
                        if c.is_whitespace() {
                            current_cpu_name = (&row[..i]).to_string();
                            previous_index = i;
                            reading_mode = ReadingMode::CPU_VALUE;
                        }
                    },
                    ReadingMode::CPU_VALUE => {
                        // Reading a number
                        if c.is_digit(32) {
                            found_number = true;
                        }
                        if found_number && (!c.is_digit(32) || i == row.len() - 1){
                            //println!("--{}--", &row[previous_index..i].trim());
                            let number: u32 = match (&row[previous_index..i]).trim().parse() {
                                Err(_) => 0,
                                Ok(n) => n
                            };
                            println!("number {}", number);
                            found_number = false;
                            previous_index = i;
                        }
                    },
                } 
            }
            /*
            if i.contains("cpu") {
                if !keyword_active {
                    keyword_active = true;
                }

                current_cpu_name = i.to_string(); // this is bad!
                if values.len() >= 7 {
                    println!("{:?}", values);
                    stats.push(ProcStatRow {
                        cpu_name: current_cpu_name,
                        softirq: values.pop().unwrap(),
                        irq: values.pop().unwrap(),
                        iowait: values.pop().unwrap(),
                        idle: values.pop().unwrap(),
                        system_proc_kernel_mode: values.pop().unwrap(),
                        nice_proc_user_mode: values.pop().unwrap(),
                        normal_proc_user_mode: values.pop().unwrap(),
                    });
                }
            }

            if i == "\n" {
                keyword_active = false;
            }

            if keyword_active {
                println!("{}", i);
                let integer: u32 = i.parse().unwrap();
                values.push(integer);
            }*/
        }
    }
    /*
        for a in stats {
            println!("cpu_name {}, normal {}, nice {}, system {}, idle {}, iowait {}, irq {}, softirq {}", a.cpu_name, a.normal_proc_user_mode, a.nice_proc_user_mode, a.system_proc_kernel_mode, a.idle, a.iowait, a.irq, a.softirq);
        }
    */
    Ok(())
}
