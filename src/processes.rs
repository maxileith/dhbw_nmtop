use regex::Regex;
use std::cmp::Ordering;
use std::fs::{read_dir, File};
use std::process::Command;
use std::str;
use std::sync::mpsc;
use std::{io::BufRead, io::BufReader};
use std::{thread, time};
use termion::event::Key;
use tui::{
    backend::Backend,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    terminal::Frame,
    widgets::{Block, Cell, Row, Table, TableState},
};

use crate::util;

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
        let _ = tx.send(ProcessList::new());
        thread::sleep(dur);
    });

    rx
}

pub struct ProcessesWidget {
    table_state: TableState,
    item_index: usize,
    sort_index: usize,
    column_index: usize,
    sort_descending: bool,
    process_list: ProcessList,
    dc_thread: mpsc::Receiver<ProcessList>,
}

impl ProcessesWidget {
    pub fn new() -> Self {
        Self {
            table_state: TableState::default(),
            item_index: 0,
            column_index: 0,
            sort_index: 0,
            sort_descending: true,
            process_list: Default::default(),
            dc_thread: init_data_collection_thread(),
        }
    }

    pub fn update(&mut self) {
        // Recv data from the data collector thread
        let processes_info = self.dc_thread.try_recv();

        if processes_info.is_ok() {
            self.process_list = processes_info.unwrap();
        }
    }

    pub fn draw<B: Backend>(&mut self, f: &mut Frame<B>, rect: Rect, block: Block) {
        let selected_style = Style::default()
            .fg(Color::Yellow)
            .bg(Color::DarkGray)
            .add_modifier(Modifier::REVERSED);
        let header_style = Style::default().bg(Color::DarkGray).fg(Color::White);
        let header_cells = [
            "PID", "PPID", "TID", "User", "Umask", "Threads", "Name", "State", "VM", "SM", "CMD",
        ]
        .iter()
        .enumerate()
        .map(|(i, h)| {
            if i == self.column_index {
                Cell::from(*h).style(Style::default().fg(Color::Yellow).bg(Color::DarkGray))
            } else {
                Cell::from(*h)
            }
        });

        let header = Row::new(header_cells).style(header_style).height(1);

        // sort list
        match self.sort_index {
            0 => {
                self.process_list
                    .processes
                    .sort_by(|a, b| a.pid.partial_cmp(&b.pid).unwrap_or(Ordering::Equal));
            }
            1 => {
                self.process_list.processes.sort_by(|a, b| {
                    a.parent_pid
                        .partial_cmp(&b.parent_pid)
                        .unwrap_or(Ordering::Equal)
                });
            }
            2 => {
                self.process_list.processes.sort_by(|a, b| {
                    a.thread_group_id
                        .partial_cmp(&b.thread_group_id)
                        .unwrap_or(Ordering::Equal)
                });
            }
            3 => {
                self.process_list
                    .processes
                    .sort_by(|a, b| a.user.partial_cmp(&b.user).unwrap_or(Ordering::Equal));
            }
            4 => {
                self.process_list
                    .processes
                    .sort_by(|a, b| a.umask.partial_cmp(&b.umask).unwrap_or(Ordering::Equal));
            }
            5 => {
                self.process_list
                    .processes
                    .sort_by(|a, b| a.threads.partial_cmp(&b.threads).unwrap_or(Ordering::Equal));
            }
            6 => {
                self.process_list
                    .processes
                    .sort_by(|a, b| a.name.partial_cmp(&b.name).unwrap_or(Ordering::Equal));
            }
            7 => {
                self.process_list
                    .processes
                    .sort_by(|a, b| a.state.partial_cmp(&b.state).unwrap_or(Ordering::Equal));
            }
            8 => {
                self.process_list.processes.sort_by(|a, b| {
                    a.virtual_memory_size
                        .partial_cmp(&b.virtual_memory_size)
                        .unwrap_or(Ordering::Equal)
                });
            }
            9 => {
                self.process_list.processes.sort_by(|a, b| {
                    a.swapped_memory
                        .partial_cmp(&b.swapped_memory)
                        .unwrap_or(Ordering::Equal)
                });
            }
            10 => {
                self.process_list
                    .processes
                    .sort_by(|a, b| a.command.partial_cmp(&b.command).unwrap_or(Ordering::Equal));
            }
            _ => {}
        }

        if self.sort_descending {
            self.process_list.processes.reverse();
        }

        let rows = self
            .process_list
            .processes
            .iter()
            //.skip(self.item_index)
            .map(|p| {
                let mut cells = Vec::new();
                cells.push(Cell::from(p.pid.to_string()));
                cells.push(Cell::from(p.parent_pid.to_string()));
                cells.push(Cell::from(p.thread_group_id.to_string()));
                cells.push(Cell::from(p.user.to_string()));
                cells.push(Cell::from(p.umask.to_string()));
                cells.push(Cell::from(p.threads.to_string()));
                cells.push(Cell::from(p.name.to_string()));
                cells.push(Cell::from(p.state.to_string()));
                cells.push(Cell::from(util::to_humanreadable(
                    p.virtual_memory_size * 1000,
                )));
                cells.push(Cell::from(util::to_humanreadable(p.swapped_memory * 1000)));
                cells.push(Cell::from(p.command.to_string()));
                Row::new(cells).height(1)
            });
        // println!("{}", rows.len());
        let table = Table::new(rows)
            .header(header)
            .highlight_style(selected_style)
            .widths(&[
                Constraint::Length(7),
                Constraint::Length(7),
                Constraint::Length(7),
                Constraint::Length(15),
                Constraint::Length(6),
                Constraint::Length(7),
                Constraint::Length(30),
                Constraint::Length(15),
                Constraint::Length(9),
                Constraint::Length(9),
                Constraint::Min(1),
            ])
            .block(block);
        f.render_stateful_widget(table, rect, &mut self.table_state);
    }

    pub fn handle_input(&mut self, key: Key) {
        match key {
            Key::Down => {
                if self.item_index < self.process_list.processes.len() - 1 {
                    self.item_index += 1;
                    self.table_state.select(Some(self.item_index));
                }
            }
            Key::Up => {
                if self.item_index > 0 {
                    self.item_index -= 1;
                    self.table_state.select(Some(self.item_index));
                }
            }
            Key::Right => {
                if self.column_index < 10 {
                    self.column_index += 1;
                }
            }
            Key::Left => {
                if self.column_index > 0 {
                    self.column_index -= 1;
                }
            }
            Key::Char('s') => {
                if self.sort_index == self.column_index {
                    self.sort_descending = !self.sort_descending;
                }

                self.sort_index = self.column_index;
            }
            _ => {}
        }
    }
}
