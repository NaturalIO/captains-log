use crate::{
    config::{LogFormat, SinkConfigTrait},
    log_impl::{LogSink, LogSinkTrait},
    rotation::*,
    time::Timer,
};
use log::{Level, Record};
use std::hash::{Hash, Hasher};
use std::os::unix::prelude::*;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::file_impl::open_file;
use crossfire::{MTx, RecvTimeoutError, Rx};
use std::thread;

/// Config for buffered file sink which merged I/O and delay flush
#[derive(Hash)]
pub struct LogBufFile {
    /// max log level in this file
    pub level: Level,

    pub format: LogFormat,

    /// path: dir/name
    pub file_path: Box<Path>,

    pub flush_millis: usize,

    pub rotation: Option<Rotation>,
}

impl LogBufFile {
    /// Construct config for file sink with buffer.
    ///
    /// Will try to create dir if not exists.
    /// Periodic flush if flush_millis is zero, or
    /// buffer size reaching 4096. will ensure a complete line write to the log file.
    ///
    /// # Arguments:
    ///
    /// The type of `dir` and `file_name` can be &str / String / &OsStr / OsString / Path / PathBuf. They can be of
    /// different types.
    ///
    /// flush_millis: default to 0, means always flush when no more message to write. when larger than
    /// zero, will wait for new message when timeout occur.
    /// the max value is 1000 (1 sec).
    pub fn new<P1, P2>(
        dir: P1, file_name: P2, level: Level, format: LogFormat, flush_millis: usize,
    ) -> Self
    where
        P1: Into<PathBuf>,
        P2: Into<PathBuf>,
    {
        let dir_path: PathBuf = dir.into();
        if !dir_path.exists() {
            std::fs::create_dir(&dir_path).expect("create dir for log");
        }
        let file_path = dir_path.join(file_name.into()).into_boxed_path();
        Self { level, format, file_path, flush_millis, rotation: None }
    }

    pub fn rotation(mut self, ro: Rotation) -> Self {
        self.rotation = Some(ro);
        self
    }
}

impl SinkConfigTrait for LogBufFile {
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
        LogSink::BufFile(LogSinkBufFile::new(self))
    }
}

pub struct LogSinkBufFile {
    max_level: Level,
    // raw fd only valid before original File close, use ArcSwap to prevent drop while using.
    formatter: LogFormat,
    th: thread::JoinHandle<()>,
    tx: MTx<Option<String>>,
}

impl LogSinkBufFile {
    pub fn new(config: &LogBufFile) -> Self {
        let (tx, rx) = crossfire::mpsc::bounded_blocking(100);

        let mut size = None;
        let mut age = None;
        if let Some(rotation) = &config.rotation {
            if let Some(_age) = &rotation.by_age {
                age.replace(RotationLimiterAge::new(*_age));
            }
            if let Some(_size) = &rotation.by_size {
                size.replace(RotationLimiterSize::new(*_size));
            }
        }
        let mut flush_millis = config.flush_millis;
        if flush_millis == 0 || flush_millis > 1000 {
            flush_millis = 1000;
        }
        let mut inner = BufFileInner {
            path: config.file_path.clone(),
            f: None,
            rx,
            size,
            age,
            flush_millis,
            buf: Vec::with_capacity(4096),
        };
        let th = thread::spawn(move || inner.log_writer());
        Self { max_level: config.level, formatter: config.format.clone(), tx, th }
    }
}

impl LogSinkTrait for LogSinkBufFile {
    fn reopen(&self) -> std::io::Result<()> {
        let _ = self.tx.send(None);
        Ok(())
    }

    #[inline(always)]
    fn log(&self, now: &Timer, r: &Record) {
        if r.level() <= self.max_level {
            // Get a stable buffer,
            // for concurrently write to file from multi process.
            let buf = self.formatter.process(now, r);
            let _ = self.tx.send(Some(buf));
        }
    }
}

/// Limit to 4k buf size, so that during reload or graceful restart,
/// the line will not be break.
const FLUSH_SIZE: usize = 4096;

struct BufFileInner {
    path: Box<Path>,
    f: Option<std::fs::File>,
    rx: Rx<Option<String>>,
    size: Option<RotationLimiterSize>,
    age: Option<RotationLimiterAge>,
    buf: Vec<u8>,
    flush_millis: usize,
}

impl BufFileInner {
    fn write(&mut self, mut s: Vec<u8>) {
        if self.buf.len() + s.len() > FLUSH_SIZE {
            if self.buf.len() > 0 {
                self.flush();
            }
        }
        self.buf.reserve(s.len());
        self.buf.append(&mut s);
        if self.buf.len() >= FLUSH_SIZE {
            self.flush();
        }
    }

    fn flush(&mut self) {
        if let Some(f) = self.f.as_ref() {
            // Use unbuffered I/O to ensure the write ok
            let _ = unsafe {
                libc::write(
                    f.as_raw_fd() as libc::c_int,
                    self.buf.as_ptr() as *const libc::c_void,
                    self.buf.len(),
                )
            };
        }
        unsafe { self.buf.set_len(0) };
    }

    fn reopen(&mut self) {
        match open_file(&self.path) {
            Ok(f) => {
                self.f.replace(f);
            }
            Err(e) => {
                eprintln!("open logfile {:#?} failed: {:?}", &self.path, e);
            }
        }
    }

    fn log_writer(&mut self) {
        self.reopen();

        macro_rules! process {
            ($msg: expr) => {
                if let Some(line) = $msg {
                    self.write(line.into());
                } else {
                    self.reopen();
                }
            };
        }
        if self.flush_millis > 0 {
            loop {
                match self.rx.recv_timeout(Duration::from_millis(self.flush_millis as u64)) {
                    Ok(msg) => {
                        process!(msg);
                        while let Ok(msg) = self.rx.try_recv() {
                            process!(msg);
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {
                        self.flush();
                    }
                    Err(RecvTimeoutError::Disconnected) => {
                        self.flush();
                        return;
                    }
                }
            }
        } else {
            loop {
                match self.rx.recv() {
                    Ok(msg) => {
                        process!(msg);
                        while let Ok(msg) = self.rx.try_recv() {
                            process!(msg);
                        }
                        self.flush();
                    }
                    Err(_) => {
                        self.flush();
                        return;
                    }
                }
            }
        }
    }
}
