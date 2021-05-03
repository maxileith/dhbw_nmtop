use regex::Regex;
use std::collections::VecDeque;
use std::fs::{read_dir, File};
use std::process::Command;
use std::str;
use std::sync::mpsc;
use std::{io::BufRead, io::BufReader};
use std::{thread, time};

#[derive(Default, Debug)]
pub struct ProcessList {
    pub processes: Vec<Process>,
}

impl ProcessList {
    pub fn new() -> Self {
        let mut new: Self = Default::default();

        let re = Regex::new("^/proc/(?P<tid>[0-9]+)$").unwrap();
        let re2 = Regex::new("^/proc/[0-9]+/task/(?P<pid>[0-9]+)$").unwrap();

        ///////////////////////////////////////
        // iterate through thread groups
        ///////////////////////////////////////
        let tg_dirs = match read_dir("/proc") {
            Ok(x) => x,
            Err(_) => return Default::default(),
        };
        for tg in tg_dirs {
            let tg = match tg {
                Ok(x) => x,
                Err(_) => return Default::default(),
            };
            let tg = tg.path();
            if tg.is_dir() {
                let tg = tg.to_str().unwrap();
                // check if dir is thread group
                if re.is_match(tg) {
                    let tid = re.captures(tg).unwrap().get(1).map_or("", |m| m.as_str());

                    ///////////////////////////////////////
                    // iterate through processes
                    ///////////////////////////////////////
                    let p_dirs = match read_dir(format!("/proc/{}/task", tid)) {
                        Ok(x) => x,
                        Err(_) => return Default::default(),
                    };
                    for p in p_dirs {
                        let p = match p {
                            Ok(x) => x,
                            Err(_) => return Default::default(),
                        };
                        let p = p.path();
                        if p.is_dir() {
                            let p = p.to_str().unwrap();
                            // check if dir is process
                            if re2.is_match(p) {
                                let pid =
                                    re2.captures(p).unwrap().get(1).map_or("", |m| m.as_str());
                                new.processes.push(Process::new(
                                    pid.parse::<usize>().unwrap(),
                                    tid.parse::<usize>().unwrap(),
                                ))
                            }
                        }
                    }
                }
            }
        }

        new
    }
}

#[derive(Default, Debug)]
pub struct Process {
    pub pid: usize,
    pub name: String,
    pub umask: String,
    pub state: String,
    pub parent_pid: usize,
    pub thread_group_id: usize,
    pub virtual_memory_size: usize,
    pub virtual_memory_size_peak: usize,
    pub swapped_memory: usize,
    pub command: String,
    pub threads: usize,
    pub user: String,
}

impl Process {
    pub fn new(pid: usize, thread_group_id: usize) -> Self {
        let mut new: Self = Default::default();
        new.pid = pid;
        new.thread_group_id = thread_group_id;
        new.update_status();
        new.update_command();
        new.update_user();
        new
    }

    fn update_status(&mut self) {
        let path: String = format!("/proc/{}/task/{}/status", self.thread_group_id, self.pid);
        let file = File::open(path);
        let filehandler = match file {
            Ok(f) => f,
            Err(_) => return,
        };
        let reader = BufReader::new(filehandler);

        for line in reader.lines() {
            let row = match line {
                Ok(x) => x,
                Err(_) => continue,
            };

            let split = row.split(':');
            let vec: Vec<&str> = split.collect();

            let value: String = vec[1].trim().to_string();
            let name: &str = vec[0].trim();

            match name {
                "Name" => (*self).name = value,
                "Umask" => (*self).umask = value,
                "State" => (*self).state = value,
                "PPid" => (*self).parent_pid = value.parse::<usize>().unwrap(),
                "VmSize" => {
                    (*self).virtual_memory_size =
                        value[0..value.len() - 3].parse::<usize>().unwrap()
                }
                "VmPeak" => {
                    (*self).virtual_memory_size_peak =
                        value[0..value.len() - 3].parse::<usize>().unwrap()
                }
                "VmSwap" => {
                    (*self).swapped_memory = value[0..value.len() - 3].parse::<usize>().unwrap()
                }
                "Threads" => (*self).threads = value.parse::<usize>().unwrap(),
                _ => continue,
            }
        }
    }

    fn update_command(&mut self) {
        let path: String = format!("/proc/{}/task/{}/cmdline", self.thread_group_id, self.pid);
        let file = File::open(path);
        let filehandler = match file {
            Ok(f) => f,
            Err(_) => return,
        };
        let reader = BufReader::new(filehandler);

        let mut result: String = String::from("");
        for line in reader.lines() {
            result = match line {
                Ok(x) => x,
                Err(_) => break,
            };
            break;
        }
        (*self).command = result;
    }

    fn update_user(&mut self) {
        let mut command = Command::new("stat");
        command.args(&[
            "-c",
            "'%U",
            &format!("/proc/{}/task/{}", self.thread_group_id, self.pid)[..],
        ]);
        let output = match command.output() {
            Ok(x) => x,
            Err(_) => panic!("Could not determine user"),
        };

        let response: &str = match str::from_utf8(&output.stdout) {
            Ok(x) => x,
            Err(e) => panic!("Invalid UTF-8 sequence: {}", e),
        };

        (*self).user = response[1..response.len() - 1].to_string();
    }
}

pub fn init_data_collection_thread() -> mpsc::Receiver<ProcessList> {
    let (tx, rx) = mpsc::channel();

    let dur = time::Duration::from_millis(5000);

    // Thread for the data collection
    let _thread = thread::spawn(move || loop {
        tx.send(ProcessList::new());
        thread::sleep(dur);
    });

    rx
}
