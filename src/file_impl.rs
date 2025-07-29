use crate::{
    config::{LogFormat, SinkConfigTrait},
    log_impl::{LogSink, LogSinkTrait},
    time::Timer,
};
use log::{Level, Record};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::{fs::OpenOptions, os::unix::prelude::*, sync::Arc};

use arc_swap::ArcSwapOption;

/// Config for file sink that supports atomic append from multiprocess.
/// For log rotation, you need system log-rotate service to notify with signal.
#[derive(Hash)]
pub struct LogRawFile {
    /// max log level in this file
    pub level: Level,

    pub format: LogFormat,

    /// path: dir/name
    pub file_path: Box<Path>,
}

impl LogRawFile {
    /// Construct config for file sink,
    /// will try to create dir if not exists.
    ///
    /// The type of `dir` and `file_name` can be &str / String / &OsStr / OsString / Path / PathBuf. They can be of
    /// different types.
    pub fn new<P1, P2>(dir: P1, file_name: P2, level: Level, format: LogFormat) -> Self
    where
        P1: Into<PathBuf>,
        P2: Into<PathBuf>,
    {
        let dir_path: PathBuf = dir.into();
        if !dir_path.exists() {
            std::fs::create_dir(&dir_path).expect("create dir for log");
        }
        let file_path = dir_path.join(file_name.into()).into_boxed_path();
        Self { level, format, file_path }
    }
}

impl SinkConfigTrait for LogRawFile {
    fn get_level(&self) -> Level {
        self.level
    }

    fn get_file_path(&self) -> Option<Box<Path>> {
        Some(self.file_path.clone())
    }

    fn write_hash(&self, hasher: &mut Box<dyn Hasher>) {
        self.hash(hasher);
        hasher.write(b"LogRawFile");
    }

    fn build(&self) -> LogSink {
        LogSink::File(LogSinkFile::new(self))
    }
}

pub(crate) struct LogSinkFile {
    max_level: Level,
    path: Box<Path>,
    // raw fd only valid before original File close, use ArcSwap to prevent drop while using.
    f: ArcSwapOption<std::fs::File>,
    formatter: LogFormat,
}

pub(crate) fn open_file(path: &Path) -> std::io::Result<std::fs::File> {
    OpenOptions::new().append(true).create(true).open(path)
}

impl LogSinkFile {
    pub fn new(config: &LogRawFile) -> Self {
        Self {
            path: config.file_path.clone(),
            max_level: config.level,
            formatter: config.format.clone(),
            f: ArcSwapOption::new(None),
        }
    }
}

impl LogSinkTrait for LogSinkFile {
    fn reopen(&self) -> std::io::Result<()> {
        match open_file(&self.path) {
            Ok(f) => {
                self.f.store(Some(Arc::new(f)));
                Ok(())
            }
            Err(e) => {
                eprintln!("open logfile {:#?} failed: {:?}", &self.path, e);
                Err(e)
            }
        }
    }

    #[inline(always)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipe;

    #[test]
    fn test_raw_file() {
        let _file_sink = LogRawFile::new("/tmp", "test.log", Level::Info, recipe::LOG_FORMAT_DEBUG);
        let dir_path = Path::new("/tmp/test_dir");
        if dir_path.is_dir() {
            std::fs::remove_dir(&dir_path).expect("ok");
        }
        let _file_sink =
            LogRawFile::new(&dir_path, "test.log", Level::Info, recipe::LOG_FORMAT_DEBUG);
        assert!(dir_path.is_dir());
        std::fs::remove_dir(&dir_path).expect("ok");
    }
}
