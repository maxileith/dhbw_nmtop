use regex::Regex;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs::{read_dir, File};
use std::process::Command;
use std::str;
use std::sync::mpsc;
use std::{io::BufRead, io::BufReader};
use std::{thread, time};
use termion::event::Key;
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    terminal::Frame,
    text::Spans,
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState, Wrap},
};

use crate::util;

/// CPUTime is used to store the most recent state of
/// the CPU time of a thread, along with a timstamp of
#[derive(Default, Clone, Copy)]
pub struct CPUTime {
    exec_time: usize,
    millis: usize,
}

impl CPUTime {
    /// Returns a new CPUTime with the given values
    pub fn new(exec_time: usize, millis: usize) -> Self {
        let mut new: Self = Default::default();
        new.exec_time = exec_time;
        new.millis = millis;
        new
    }
}

/// ProcessList not only stores the list of processes (or threads),
/// but also CPUTime's of the threads to make it possible to calculate
/// the CPU usage.
#[derive(Default)]
pub struct ProcessList {
    cpu_times: HashMap<usize, CPUTime>,
    pub processes: Vec<Process>,
}

impl ProcessList {
    /// Returns a blank ProcessList
    ///
    /// # Panic
    ///
    /// This function won't panic.
    pub fn new() -> Self {
        Default::default()
    }

    /// Creates a deep copy of a ProcessList
    ///
    /// # Panic
    ///
    /// This function won't panic.
    pub fn clone(&mut self) -> Self {
        let mut new: Self = Default::default();
        new.cpu_times = self.cpu_times.clone();
        let mut processes: Vec<Process> = Default::default();
        for p in self.processes.iter() {
            processes.push(p.clone());
        }
        new.processes = processes;
        new
    }

    /// Update everything contained by the list of processes
    ///
    /// This function deletes the current list of processes and
    /// replaces it by a new one with uptodate metrics.
    ///
    /// # Panic
    ///
    /// This function won't panic.
    pub fn update(&mut self) {
        self.processes = Default::default();

        let re_pid = Regex::new("^/proc/(?P<pid>[0-9]+)$").unwrap();
        let re_tid = Regex::new("^/proc/[0-9]+/task/(?P<tid>[0-9]+)$").unwrap();

        // get to know all possible process directories
        let pid_dirs = match read_dir("/proc") {
            Ok(x) => x,
            Err(_) => return,
        };
        ///////////////////////////////////////
        // iterate processes
        ///////////////////////////////////////
        for pid in pid_dirs {
            let pid = match pid {
                Ok(x) => x.path(),
                Err(_) => continue,
            };
            // the path has to be a directory to be a process
            if !pid.is_dir() {
                continue;
            }
            // convert pid to string
            let pid = match pid.to_str() {
                Some(x) => x,
                _ => continue,
            };
            // check if directory is a process by checking against the expected pattern
            if !re_pid.is_match(pid) {
                continue;
            }
            // get process id from regex
            let pid = match re_pid.captures(pid) {
                Some(x) => x.get(1).map_or("", |m| m.as_str()),
                _ => continue,
            };
            // get to know all threads of the process
            let tid_dirs = match read_dir(format!("/proc/{}/task", pid)) {
                Ok(x) => x,
                Err(_) => continue,
            };
            // to integer
            let pid = match pid.parse::<usize>() {
                Ok(x) => x,
                Err(_) => continue,
            };
            ///////////////////////////////////////
            // iterate through threads
            ///////////////////////////////////////
            for tid in tid_dirs {
                let tid = match tid {
                    Ok(x) => x.path(),
                    Err(_) => continue,
                };
                // the path has to be a directory to be a thread
                if !tid.is_dir() {
                    continue;
                }
                // convert tid to string
                let tid = match tid.to_str() {
                    Some(x) => x,
                    _ => continue,
                };
                // check if directory is a thread by checking against the expected pattern
                if !re_tid.is_match(tid) {
                    continue;
                }
                // get thread id from regex
                let tid = match re_tid.captures(tid) {
                    Some(x) => x.get(1).map_or("", |m| m.as_str()),
                    _ => continue,
                };
                ///////////////////////////////
                // Found thread -> add to list
                ///////////////////////////////
                let tid = match tid.parse::<usize>() {
                    Ok(x) => x,
                    Err(_) => continue,
                };
                self.processes
                    .push(Process::new(pid, tid, &mut self.cpu_times))
            }
        }
    }
}

/// Process is used to store information of one
/// Process (or thread)
#[derive(Default, Debug, Clone)]
pub struct Process {
    pub pid: usize,
    pub name: String,
    pub umask: String,
    pub state: String,
    pub parent_pid: usize,
    pub tid: usize,
    pub memory: usize,
    pub command: String,
    pub threads: usize,
    pub user: String,
    pub nice: i8,
    cpu_time: usize,
    pub cpu_usage: f32,
}

impl Process {
    /// Create a Process (or thread) with current metrics
    ///
    /// This function returns a Process with current metrics
    ///
    /// # Arguments
    ///
    /// * `pid` - the process id of the process (or thread) that is to be investigated
    /// * `tid` - the thread id of the thread that is to be investigated
    /// * `cpu_times` - map of CPU times to calculate the CPU usage
    ///
    /// # Panic
    ///
    /// This function won't panic.
    pub fn new(pid: usize, tid: usize, cpu_times: &mut HashMap<usize, CPUTime>) -> Self {
        let mut new: Self = Default::default();
        new.pid = pid;
        new.tid = tid;
        new.update(cpu_times);
        new
    }

    /// Update the Process (or thread)
    ///
    /// This function updates every attribute of the process (or thread)
    /// to match the current state
    ///
    /// # Arguments
    ///
    /// * `cpu_times` - map of CPU times to calculate the CPU usage
    ///
    /// # Panic
    ///
    /// This function won't panic.
    pub fn update(&mut self, cpu_times: &mut HashMap<usize, CPUTime>) {
        self.update_status();
        self.update_command();
        self.update_user();
        self.update_stat();
        self.update_cpu_usage(cpu_times);
    }

    /// Update the Process (or thread) status
    ///
    /// This function updates every attribute of the process (or thread)
    /// that is read from '/proc/[pid]/task/[tid]/status'.
    ///
    /// # Updates the following attributes:
    ///
    /// * `name`
    /// * `umask`
    /// * `memory`
    ///
    /// # Panic
    ///
    /// This function won't panic.
    fn update_status(&mut self) {
        let path: String = format!("/proc/{}/task/{}/status", self.pid, self.tid);
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

            // https://man7.org/linux/man-pages/man5/proc.5.html
            match name {
                "Name" => (*self).name = value,
                "Umask" => (*self).umask = value,
                "RssAnon" => {
                    // value.len() - 3 cuts of " KB" at the end of the string
                    let value = match value[0..value.len() - 3].parse::<usize>() {
                        Ok(x) => x,
                        Err(_) => 0,
                    };
                    (*self).memory = value;
                    // 'RssAnon" is the last value that is needed -> break
                    break;
                }
                _ => continue,
            }
        }
    }

    /// Update the Process (or thread) command
    ///
    /// This function updates the command that the process (or thread) was started
    /// with. From '/proc/[pid]/task/[tid]/cmdline'.
    ///
    /// # Updates the following attributes:
    ///
    /// * `command`
    ///
    /// # Panic
    ///
    /// This function won't panic.
    fn update_command(&mut self) {
        // https://man7.org/linux/man-pages/man5/proc.5.html
        let path: String = format!("/proc/{}/task/{}/cmdline", self.pid, self.tid);
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

    /// Update the Process (or thread) user
    ///
    /// This function updates the user that started the process
    /// (or thread).
    ///
    /// # Updates the following attributes:
    ///
    /// * `user`
    ///
    /// # Panic
    ///
    /// This function won't panic.
    fn update_user(&mut self) {
        let mut command = Command::new("stat");
        command.args(&[
            "-c",
            "'%U",
            &format!("/proc/{}/task/{}", self.pid, self.tid)[..],
        ]);
        let output = match command.output() {
            Ok(x) => x,
            Err(_) => return,
        };

        let response: &str = match str::from_utf8(&output.stdout) {
            Ok(x) => x,
            Err(_) => "Invalid",
        };

        (*self).user = response[1..response.len() - 1].to_string();
    }

    /// Update the Process (or thread) stat
    ///
    /// This function updates every attribute of the process (or thread)
    /// that is read from '/proc/[pid]/task/[tid]/stat'.
    ///
    /// # Updates the following attributes:
    ///
    /// * `state`
    /// * `parent_pid`
    /// * `nice`
    /// * `threads`
    /// * `cpu_time`
    ///
    /// # Panic
    ///
    /// This function won't panic.
    fn update_stat(&mut self) {
        // https://man7.org/linux/man-pages/man5/proc.5.html
        let path: String = format!("/proc/{}/task/{}/stat", self.pid, self.tid);
        let file = File::open(path);
        let filehandler = match file {
            Ok(f) => f,
            Err(_) => return,
        };
        let mut reader = BufReader::new(filehandler);

        let mut result = String::new();
        let _ = reader.read_line(&mut result);

        // Example of result:
        // 2180 (JS Helper) S 2078 2166 2166 0 -1 1077936192 1468600 6667190 0 4242 310 106 6477 18537 20 0 13 0 1944 4942053376 180392 18446744073709551615 1 1 0 0 0 0 0 16781312 83128 0 0 0 -1 18 0 0 0 0 0 0 0 0 0 0 0 0 0
        //
        // --> start behind ")" because --Space-- in (JS Helper) does mess up things and information before is not needed anyway
        let tmp: Vec<&str> = result.split(") ").collect();
        let metrics: Vec<&str> = tmp[1].split(" ").collect();

        // https://man7.org/linux/man-pages/man5/proc.5.html
        (*self).state = metrics[0].to_string();
        (*self).parent_pid = match metrics[1].parse::<usize>() {
            Ok(x) => x,
            Err(_) => 0,
        };
        (*self).nice = match metrics[16].parse::<i8>() {
            Ok(x) => x,
            Err(_) => 0,
        };
        (*self).threads = match metrics[17].parse::<usize>() {
            Ok(x) => x,
            Err(_) => 0,
        };
        let utime = match metrics[11].parse::<usize>() {
            Ok(x) => x,
            Err(_) => 0,
        };
        let stime = match metrics[12].parse::<usize>() {
            Ok(x) => x,
            Err(_) => 0,
        };
        (*self).cpu_time = utime + stime;
    }

    /// Calculates the cpu usage
    ///
    /// This function calculates the CPU usage of the process (or thread)
    /// by using the cpu_times list and the current state.
    ///
    /// # Updates the following attributes:
    ///
    /// * `cpu_usage`
    ///
    /// # Arguments
    ///
    /// * `cpu_times` - map of CPU times to calculate the CPU usage
    ///
    /// # Panic
    ///
    /// This function won't panic.
    fn update_cpu_usage(&mut self, cpu_times: &mut HashMap<usize, CPUTime>) {
        // get cpu time of the process (or thread) from last time it was updated
        let old_cpu_times = match cpu_times.get(&self.tid) {
            Some(x) => *x,
            None => Default::default(),
        };
        // calculate the elapsed cpu time of the process (or thread) in Linux ticks (default: 100/s)
        let delta_cpu_time: f32 = (self.cpu_time - old_cpu_times.exec_time) as f32;
        // calculate the (real) elapsed time (in seconds)
        let delta_real_time: f32 =
            ((util::get_millis() - old_cpu_times.millis) as f64 / 1000.0) as f32;

        // update the values of the HashMap
        match cpu_times.get_mut(&self.tid) {
            Some(x) => {
                // if there was already a value for the process (or thread),
                // just overwrite it with the current one
                *x = CPUTime::new(self.cpu_time, util::get_millis());
            }
            None => {
                // otherwise insert a new one
                cpu_times.insert(self.tid, CPUTime::new(self.cpu_time, util::get_millis()));
                ()
            }
        }

        // Because delta_cpu_time is calculated in Linux ticks (default: 100/s),
        // it is not necessary to multiply 100 to the result to get a percentage value.
        (*self).cpu_usage = delta_cpu_time / delta_real_time;
    }
}

/// Initializes a thread to collect and send the process list each 2.5 seconds.
///
/// The ProcessList is created once and updated on every iteration.
///
/// # Panic
///
/// This function won't panic.
pub fn init_data_collection_thread() -> mpsc::Receiver<ProcessList> {
    let (tx, rx) = mpsc::channel();

    let dur = time::Duration::from_millis(2500);

    let mut pl: ProcessList = ProcessList::new();

    // Thread for the data collection
    let _ = thread::spawn(move || loop {
        pl.update();
        // Send a clone to keep the ownership
        let _ = tx.send(pl.clone());
        thread::sleep(dur);
    });

    rx
}

#[derive(PartialEq)]
enum InputMode {
    Niceness,
    Filter,
}

pub struct ProcessesWidget {
    table_state: TableState,
    item_index: usize,
    sort_index: usize,
    column_index: usize,
    filter_index: Option<usize>,
    filter_value_str: String,
    filter_value_usize: usize,
    sort_descending: bool,
    process_list: ProcessList,
    dc_thread: mpsc::Receiver<ProcessList>,
    popup_open: bool,
    input: String,
    input_mode: InputMode,
}

impl ProcessesWidget {
    pub fn new() -> Self {
        let mut a = Self {
            table_state: TableState::default(),
            item_index: 0,
            column_index: 9,
            sort_index: 9,
            sort_descending: true,
            process_list: Default::default(),
            dc_thread: init_data_collection_thread(),
            popup_open: false,
            input: String::from(""),
            input_mode: InputMode::Niceness,
            filter_index: None,
            filter_value_str: String::from(""),
            filter_value_usize: 0,
        };
        a.table_state.select(Some(0));
        a
    }

    fn sort(&mut self) {
        let sort_index = self.sort_index;
        let sort_descending = self.sort_descending;
        self.process_list.processes.sort_by(|a, b| {
            let s = match sort_index {
                0 => a.pid.partial_cmp(&b.pid).unwrap_or(Ordering::Equal),
                1 => a
                    .parent_pid
                    .partial_cmp(&b.parent_pid)
                    .unwrap_or(Ordering::Equal),
                2 => a.tid.partial_cmp(&b.tid).unwrap_or(Ordering::Equal),
                3 => a.user.partial_cmp(&b.user).unwrap_or(Ordering::Equal),
                4 => a.umask.partial_cmp(&b.umask).unwrap_or(Ordering::Equal),
                5 => a.threads.partial_cmp(&b.threads).unwrap_or(Ordering::Equal),
                6 => a.name.partial_cmp(&b.name).unwrap_or(Ordering::Equal),
                7 => a.state.partial_cmp(&b.state).unwrap_or(Ordering::Equal),
                8 => a.nice.partial_cmp(&b.nice).unwrap_or(Ordering::Equal),
                9 => a
                    .cpu_usage
                    .partial_cmp(&b.cpu_usage)
                    .unwrap_or(Ordering::Equal),
                10 => a.memory.partial_cmp(&b.memory).unwrap_or(Ordering::Equal),
                11 => a.command.partial_cmp(&b.command).unwrap_or(Ordering::Equal),
                _ => Ordering::Equal,
            };
            
            if sort_descending {
                Ordering::reverse(s)
            } else {
                s
            }
        });
    }

    fn filter(&self, p: &Process) -> bool {
        match self.filter_index {
            // Numbers
            Some(0) => p.pid == self.filter_value_usize,
            Some(1) => p.parent_pid == self.filter_value_usize,
            Some(2) => p.tid == self.filter_value_usize,
            Some(5) => p.threads == self.filter_value_usize,
            // Strings
            Some(3) => p.user.contains(&self.filter_value_str),
            Some(4) => p.umask.contains(&self.filter_value_str),
            Some(6) => p.name.contains(&self.filter_value_str),
            Some(7) => p.state.contains(&self.filter_value_str),
            Some(11) => p.command.contains(&self.filter_value_str),
            _ => true,
        }
    }

    pub fn update(&mut self) {
        // Recv data from the data collector thread
        let processes_info = self.dc_thread.try_recv();

        match processes_info {
            Ok(x) => {
                if !self.popup_open {
                    self.process_list = x;
                    self.sort();
                }
            }
            Err(_) => (),
        }
    }

    pub fn draw<B: Backend>(&mut self, f: &mut Frame<B>, rect: Rect, block: Block) {
        let selected_style = Style::default()
            .fg(Color::White)
            .bg(Color::DarkGray)
            .add_modifier(Modifier::REVERSED);
        let header_style = Style::default().bg(Color::DarkGray).fg(Color::White);
        let header_cells = [
            "PID", "PPID", "TID", "User", "Umask", "Threads", "Name", "State", "Nice", "CPU",
            "Mem", "CMD",
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

        let rows = self
            .process_list
            .processes
            .iter()
            .filter(|p| self.filter(p))
            .map(|p| {
                let mut cells = Vec::new();
                cells.push(Cell::from(format!("{: >7}", p.pid)));
                cells.push(Cell::from(format!("{: >7}", p.parent_pid)));
                cells.push(Cell::from(format!("{: >7}", p.tid)));
                cells.push(Cell::from(p.user.to_string()));
                cells.push(Cell::from(format!("{: >5}", p.umask)));
                cells.push(Cell::from(format!("{: >7}", p.threads)));
                cells.push(Cell::from(p.name.to_string()));
                cells.push(Cell::from(p.state.to_string()));
                cells.push(Cell::from(format!("{: >4}", p.nice)));
                cells.push(Cell::from(format!(
                    "{: >7}",
                    format!("{:3.2}%", p.cpu_usage)
                )));
                cells.push(Cell::from(format!(
                    "{: >9}",
                    util::to_humanreadable(p.memory * 1024)
                )));
                cells.push(Cell::from(p.command.to_string()));
                Row::new(cells).height(1)
            });
        let table = Table::new(rows)
            .header(header)
            .highlight_style(selected_style)
            .widths(&[
                Constraint::Length(8),
                Constraint::Length(8),
                Constraint::Length(8),
                Constraint::Length(15),
                Constraint::Length(6),
                Constraint::Length(7),
                Constraint::Length(30),
                Constraint::Length(6),
                Constraint::Length(5),
                Constraint::Length(8),
                Constraint::Length(9),
                Constraint::Min(1),
            ])
            .block(block);
        f.render_stateful_widget(table, rect, &mut self.table_state);

        if self.popup_open {
            self.draw_popup(f, &rect);
        }
    }

    fn draw_popup<B: Backend>(&mut self, f: &mut Frame<B>, rect: &Rect) {
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(
                [
                    Constraint::Percentage(25),
                    Constraint::Percentage(50),
                    Constraint::Percentage(25),
                ]
                .as_ref(),
            )
            .split(*rect);

        let popup = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Length((rect.height - 8) / 2),
                    Constraint::Length(8),
                    Constraint::Min((rect.height - 8) / 2),
                ]
                .as_ref(),
            )
            .split(horizontal[1]);

        let clear = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Length((rect.height - 10) / 2),
                    Constraint::Length(10),
                    Constraint::Min((rect.height - 10) / 2),
                ]
                .as_ref(),
            )
            .split(horizontal[1]);

        let text = vec![
            Spans::default(),
            Spans::from(format!("{}", self.input)),
            Spans::default(),
            Spans::default(),
            Spans::from("(C)ancel"),
            Spans::from("Press Enter to apply"),
        ];
        let block = Block::default()
            .style(Style::default().fg(Color::Yellow))
            .title("Input")
            .borders(Borders::ALL);
        let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: true });
        f.render_widget(Clear, clear[1]);
        f.render_widget(paragraph, popup[1]);
    }

    pub fn handle_input(&mut self, key: Key) {
        if !self.popup_open {
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
                    if self.column_index < 12 {
                        self.column_index += 1;
                    }
                }
                Key::Char('f') => {
                    self.input_mode = InputMode::Filter;
                    self.popup_open = !self.popup_open;
                }
                Key::Char('r') => {
                    self.filter_index = None;
                }
                Key::Char('k') => {
                    util::kill_process(self.process_list.processes[self.item_index].tid)
                }
                Key::Char('n') => {
                    self.input_mode = InputMode::Niceness;
                    self.popup_open = !self.popup_open;
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
                    self.sort();
                }
                _ => {}
            }
        } else {
            match key {
                Key::Backspace => {
                    self.input.pop();
                }
                Key::Char('\n') => {
                    let input_value = self.input.parse().unwrap_or_default();

                    if self.input_mode == InputMode::Niceness {
                        util::update_niceness(
                            self.process_list.processes[self.item_index].tid,
                            input_value,
                        );
                    } else if self.input_mode == InputMode::Filter {
                        self.filter_index = Some(self.column_index);
                        match self.filter_index {
                            Some(i) => {
                                if self.is_usize_column(i) {
                                    let input_value: usize = self.input.parse().unwrap_or_default();
                                    self.filter_value_usize = input_value;
                                } else if self.is_string_column(i) {
                                    let input_value: String =
                                        self.input.parse().unwrap_or_default();
                                    self.filter_value_str = input_value;
                                }
                            }
                            None => {}
                        }
                    }
                    self.input.clear();
                    self.popup_open = false;
                }
                Key::Char('c') => {
                    self.input.clear();
                    self.popup_open = false;
                }
                Key::Char(key) => {
                    if self.input_mode == InputMode::Filter {
                        self.input.push(key)
                    } else {
                        if self.input.len() < 3 {
                            self.input.push(key)
                        }
                    }
                }
                Key::Esc => {
                    self.input.clear();
                    self.popup_open = false;
                }
                _ => {}
            }
        }
    }

    fn is_usize_column (&self, v: usize) -> bool {
        v <= 2 || v == 5
        
    }

    fn is_string_column (&self, v: usize) -> bool {
        v == 3 || v == 6 || v == 7 || v == 11 || v == 4

    }

    pub fn get_help_text(&self) -> &str {
        let i = self.column_index;
        match self.filter_index {
            Some(i) => {
                if self.is_string_column(i) || self.is_usize_column(i) {
                    ", f: filter, r: reset filter"
                } else {
                    ", r: reset filter"
                }
            }
            None => {
                if self.is_string_column(i) || self.is_usize_column(i) {
                    ", f: filter"
                } else {
                    ""
                }
            }
        }
    }
}
