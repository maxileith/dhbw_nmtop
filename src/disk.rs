use std::time;
use std::thread;
use std::sync::mpsc;
use std::process::Command;
use std::str;

// equals the "df"-command output
#[derive(Debug)]
pub struct DiskInfo {
    pub filesystem: String,
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub used_percentage: String,
    pub mountpoint: String,
}

pub fn get_disks_usage() -> Vec<DiskInfo> {
    let mut disk_array = Vec::new();
    // execute "df"
    let mut df_command = Command::new("df");
    let df_output = match df_command.output() {
        Ok(x) => x,
        _ => panic!("Could not read df output"),
    };

    // parse string from utf8 Vec
    let df_output_string = match str::from_utf8(&df_output.stdout) {
        Ok(v) => v,
        Err(e) => panic!("Invalid UTF-8 sequence: {}", e),
    };

    // add disks to array
    for line in df_output_string.lines() {
        //println!("{}", line.starts_with("/dev/"));

        if line.starts_with("/dev/") {
            let mut sliced_line = line.split_whitespace();
            let disk_info = DiskInfo {
                filesystem : sliced_line.next().unwrap().replace("/dev", "").to_string(),
                total: sliced_line.next().unwrap().parse().unwrap(),
                used: sliced_line.next().unwrap().parse().unwrap(),
                available: sliced_line.next().unwrap().parse().unwrap(),
                used_percentage: sliced_line.next().unwrap().to_string(),
                mountpoint: sliced_line.next().unwrap().to_string(),
            };

            disk_array.push(disk_info);
        }
    }
    //println!("{:?}", disk_array);

    disk_array
}


pub fn init_data_collection_thread() -> mpsc::Receiver<Vec<DiskInfo>> {
  let (tx, rx) = mpsc::channel();
  let dur = time::Duration::from_millis(100);

  // Thread for the data collection
  let dc_thread = thread::spawn(move || loop {
      let m = get_disks_usage();
      
      tx.send(m);

      thread::sleep(dur);
  });

  rx
}