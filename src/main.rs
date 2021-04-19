use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader};
use std::vec::Vec;
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

#[derive(PartialEq)]
enum ReadingMode {
    CpuName,
    CpuValue,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proc_stat = "/proc/stat";
    println!("Reading from {}", proc_stat);


    //for x in 0..11{
        let file = File::open(proc_stat)?;
        let reader = BufReader::new(file);
        let mut keyword_active = false;
        let mut stats: Vec<ProcStatRow> = Vec::new();

        let mut reading_mode = ReadingMode::CpuName;
        let mut previous_index = 0;
        let mut found_number = false;
        
        for line in reader.lines() {
            let row = line?;
            if row.starts_with("cpu") {
                previous_index = 0;
                reading_mode = ReadingMode::CpuName;

                //let mut stat: ProcStatRow;
                let mut current_cpu_name: &str = "";
                let mut values: [u32; 10] = [0; 10];
                let mut field_counter = 0;

                for (i, c) in row.chars().enumerate() {
                    if reading_mode == ReadingMode::CpuName {
                        if c.is_whitespace() {
                            current_cpu_name = &row[..i];
                            previous_index = i;
                            reading_mode = ReadingMode::CpuValue;
                        }
                    } else if reading_mode == ReadingMode::CpuValue {
                        // Reading a number
                        if c.is_digit(32) {
                            found_number = true;
                        }
                        if found_number && (!c.is_digit(32) || i == row.chars().count() - 1) {
                            //println!("--{}--", &row[previous_index..i].trim());
                            let number: u32 = match (&row[previous_index..i]).trim().parse() {
                                Err(_) => 0,
                                Ok(n) => n,
                            };
                            values[field_counter] = number;
                            
                            found_number = false;
                            previous_index = i;

                            // Found new field
                            field_counter += 1;
                        }
                    }
                }

                // Create struct from numbers
                stats.push(ProcStatRow {
                    cpu_name: current_cpu_name.to_string(), // ugly, should find better way
                    softirq: values[6],
                    irq: values[5],
                    iowait: values[4],
                    idle: values[3],
                    system_proc_kernel_mode: values[2],
                    nice_proc_user_mode: values[1],
                    normal_proc_user_mode: values[0],
                });
            }
        }
        //let dur = time::Duration::from_millis(500);
        //thread::sleep(dur);
        for a in stats {
            println!("cpu_name {}, normal {}, nice {}, system {}, idle {}, iowait {}, irq {}, softirq {}", a.cpu_name, a.normal_proc_user_mode, a.nice_proc_user_mode, a.system_proc_kernel_mode, a.idle, a.iowait, a.irq, a.softirq);
        }
    //}
    Ok(())
}
