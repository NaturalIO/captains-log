use crate::{config::LogFile, formatter::LogFormat, log_impl::LoggerSinkTrait, time::Timer};
use log::{Level, Record};
use std::{fs::OpenOptions, os::unix::prelude::*, path::Path, sync::Arc};

use arc_swap::ArcSwapOption;

pub struct LoggerSinkFile {
    max_level: Level,
    path: Box<Path>,
    // raw fd only valid before original File close, use ArcSwap to prevent drop while using.
    f: ArcSwapOption<std::fs::File>,
    formatter: LogFormat,
}

fn open_file(path: &Path) -> std::io::Result<std::fs::File> {
    OpenOptions::new().append(true).create(true).open(path)
}

impl LoggerSinkFile {
    pub fn new(config: &LogFile) -> Self {
        Self {
            path: config.file_path.clone(),
            max_level: config.level,
            formatter: config.format.clone(),
            f: ArcSwapOption::new(None),
        }
    }
}

impl LoggerSinkTrait for LoggerSinkFile {
    fn reopen(&self) -> std::io::Result<()> {
        match open_file(&self.path) {
            Ok(f) => {
                println!("reopen {:#?}", &self.path);
                self.f.store(Some(Arc::new(f)));
                Ok(())
            }
            Err(e) => {
                println!("reopen logfile {:#?} failed: {:?}", &self.path, e);
                Err(e)
            }
        }
    }

    fn log(&self, now: &Timer, r: &Record) {
        if r.level() <= self.max_level {
            // ArcSwap ensure file fd is not close during reopen for log rotation,
            // in case of panic during write.
            if let Some(file) = self.f.load_full() {
                // Get a stable buffer,
                // for concurrently write to file from multi process.
                let buf = self.formatter.process(now, r);
                unsafe {
                    let _ = libc::write(
                        file.as_raw_fd() as libc::c_int,
                        buf.as_ptr() as *const libc::c_void,
                        buf.len(),
                    );
                }
            }
        }
    }
}
