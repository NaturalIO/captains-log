use crate::{Builder, parser::LogParser};
use std::fs::remove_file;

pub const TEST_LOCK_FILE: &str = "/tmp/natualio_test_lock";

macro_rules! lock_file {
    () => {
        // NOTE: use one {} to expose the guard into context
        let lock_fd = OpenOptions::new().create(true).write(true).open(&TEST_LOCK_FILE).unwrap();
        let _guard = fmutex::lock_exclusive(&lock_fd).unwrap();
    };
}
pub(super) use lock_file;

pub fn clear_test_files(builder: &Builder) {
    for sink in &builder.sinks {
        if let Some(file_path) = sink.get_file_path() {
            let _ = remove_file(file_path);
        }
    }
}

pub fn parse_log(file_path: &str, re: &str) -> std::io::Result<Vec<Vec<String>>> {
    let parser = LogParser::new(file_path, re, 1024)?;
    let mut lines = Vec::with_capacity(1024);
    for line in parser.lines() {
        let _line = line?;
        lines.push(_line);
    }
    Ok(lines)
}
