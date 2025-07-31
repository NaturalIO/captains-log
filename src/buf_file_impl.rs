use crate::{
    config::{LogFormat, SinkConfigTrait},
    log_impl::{LogSink, LogSinkTrait},
    rotation::*,
    time::Timer,
};
use log::{Level, Record};
use std::fs::metadata;
use std::hash::{Hash, Hasher};
use std::os::unix::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::Once;
use std::time::{Duration, SystemTime};

use crate::file_impl::open_file;
use crossfire::{MTx, RecvTimeoutError, Rx};
use std::thread;

/// Limit to 4k buf size, so that during reload or graceful restart,
/// the line will not be break.
pub const FLUSH_SIZE_DEFAULT: usize = 4096;

/// Config for buffered file sink which merged I/O and delay flush.
/// Optional log rotation can be configured.
///
/// Used when you don't have a SSD and the log is massive.
///
/// **When your program shutting down, should call flush to ensure the log is written to disk.**
///
/// ``` rust
/// log::logger().flush();
/// ```
/// On panic, our panic hook will call `flush()` explicitly.
///
/// flush size default to be 4k to prevent line breaks on program (graceful) restart.
///
/// # Example
///
/// Source of [crate::recipe::buffered_file_logger_custom()]
///
/// ``` rust
/// use captains_log::*;
/// use std::path::{self, Path, PathBuf};
///
/// pub fn buffered_file_logger_custom<P: Into<PathBuf>>(
///     file_path: P, max_level: Level, time_fmt: &'static str, format_func: FormatFunc,
///     flush_millis: usize, rotate: Option<Rotation>,
/// ) -> Builder {
///     let format = LogFormat::new(time_fmt, format_func);
///     let _file_path = file_path.into();
///     let p = path::absolute(&_file_path).expect("path convert to absolute");
///     let dir = p.parent().unwrap();
///     let file_name = Path::new(p.file_name().unwrap());
///     let mut file = LogBufFile::new(dir, file_name, max_level, format, flush_millis);
///     if let Some(ro) = rotate {
///         file = file.rotation(ro);
///     }
///     let mut config = Builder::default().signal(signal_hook::consts::SIGUSR1).buf_file(file);
///     // panic on debugging
///     #[cfg(debug_assertions)]
///     {
///         config.continue_when_panic = false;
///     }
///     // do not panic on release
///     #[cfg(not(debug_assertions))]
///     {
///         config.continue_when_panic = true;
///     }
///     return config;
/// }
///```
#[derive(Hash)]
pub struct LogBufFile {
    /// max log level in this file
    pub level: Level,

    pub format: LogFormat,

    /// path: dir/name
    pub file_path: Box<Path>,

    /// default to 0, means always flush when no more message to write.
    ///
    /// when larger than zero, will wait for new message when timeout occur.
    ///
    /// Max value is 1000 (1 sec).
    pub flush_millis: usize,

    /// Rotation config
    pub rotation: Option<Rotation>,

    /// Auto flush when buffer size is reached, default to be 4k
    pub flush_size: usize,
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
    /// - `flush_millis`:
    ///
    ///    - default to 0, means always flush when no more message to write.
    ///
    ///    - when larger than zero, will wait for new message when timeout occur.
    /// The max value is 1000 (1 sec).
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
        Self {
            level,
            format,
            file_path,
            flush_millis,
            rotation: None,
            flush_size: FLUSH_SIZE_DEFAULT,
        }
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
        hasher.write(b"LogBufFile");
    }

    fn build(&self) -> LogSink {
        LogSink::BufFile(LogSinkBufFile::new(self))
    }
}

pub(crate) struct LogSinkBufFile {
    max_level: Level,
    // raw fd only valid before original File close, use ArcSwap to prevent drop while using.
    formatter: LogFormat,
    _th: thread::JoinHandle<()>,
    tx: MTx<Msg>,
}

impl LogSinkBufFile {
    pub fn new(config: &LogBufFile) -> Self {
        let (tx, rx) = crossfire::mpsc::bounded_blocking(100);

        let mut flush_millis = config.flush_millis;
        if flush_millis == 0 || flush_millis > 1000 {
            flush_millis = 1000;
        }
        let mut rotate_impl: Option<LogRotate> = None;
        if let Some(r) = &config.rotation {
            rotate_impl = Some(r.build(&config.file_path));
        }
        let mut flush_size = config.flush_size;
        if flush_size == 0 {
            flush_size = FLUSH_SIZE_DEFAULT;
        }
        let mut inner = BufFileInner {
            size: 0,
            create_time: None,
            path: config.file_path.to_path_buf(),
            f: None,
            flush_millis,
            flush_size,
            buf: Vec::with_capacity(4096),
            rotate: rotate_impl,
        };
        let _th = thread::spawn(move || inner.log_writer(rx));
        Self { max_level: config.level, formatter: config.format.clone(), tx, _th }
    }
}

impl LogSinkTrait for LogSinkBufFile {
    fn reopen(&self) -> std::io::Result<()> {
        let _ = self.tx.send(Msg::Reopen);
        Ok(())
    }

    #[inline(always)]
    fn log(&self, now: &Timer, r: &Record) {
        if r.level() <= self.max_level {
            // Get a stable buffer,
            // for concurrently write to file from multi process.
            let buf = self.formatter.process(now, r);
            let _ = self.tx.send(Msg::Line(buf));
        }
    }

    #[inline(always)]
    fn flush(&self) {
        let _ = self.tx.send(Msg::Flush(Once::new()));
    }
}

enum Msg {
    Line(String),
    Reopen,
    Flush(Once),
}

struct BufFileInner {
    size: u64,
    create_time: Option<SystemTime>,
    path: PathBuf,
    f: Option<std::fs::File>,
    buf: Vec<u8>,
    flush_millis: usize,
    rotate: Option<LogRotate>,
    flush_size: usize,
}

impl FileSinkTrait for BufFileInner {
    #[inline(always)]
    fn get_create_time(&self) -> SystemTime {
        self.create_time.unwrap()
    }

    #[inline(always)]
    fn get_size(&self) -> u64 {
        self.size
    }
}

impl BufFileInner {
    fn reopen(&mut self) {
        match open_file(&self.path) {
            Ok(f) => {
                let mt = metadata(&self.path).expect("get metadata");
                self.size = mt.len();
                if self.create_time.is_none() {
                    // NOTE Posix has no create_time, so use mtime. rotation will delay a cycle after program restart.
                    self.create_time = Some(mt.modified().unwrap());
                }
                self.f.replace(f);
            }
            Err(e) => {
                eprintln!("open logfile {:#?} failed: {:?}", &self.path, e);
            }
        }
    }

    fn write(&mut self, mut s: Vec<u8>) {
        if self.buf.len() + s.len() > self.flush_size {
            if self.buf.len() > 0 {
                self.flush(false);
            }
        }
        self.buf.reserve(s.len());
        self.buf.append(&mut s);
        if self.buf.len() >= self.flush_size {
            self.flush(false);
        }
    }

    #[inline(always)]
    fn check_rotate(&mut self) {
        if let Some(ro) = self.rotate.as_ref() {
            if ro.rotate(self) {
                self.reopen();
            }
        }
    }

    fn flush(&mut self, wait_rotate: bool) {
        if let Some(f) = self.f.as_ref() {
            self.size += self.buf.len() as u64;
            // Use unbuffered I/O to ensure the write ok
            let _ = unsafe {
                libc::write(
                    f.as_raw_fd() as libc::c_int,
                    self.buf.as_ptr() as *const libc::c_void,
                    self.buf.len(),
                )
            };
            unsafe { self.buf.set_len(0) };
            self.check_rotate();
        }
        if wait_rotate {
            if let Some(ro) = self.rotate.as_ref() {
                ro.wait();
            }
        }
    }

    fn log_writer(&mut self, rx: Rx<Msg>) {
        self.reopen();
        self.check_rotate();

        macro_rules! process {
            ($msg: expr) => {
                match $msg {
                    Msg::Line(line) => {
                        self.write(line.into());
                    }
                    Msg::Reopen => {
                        self.reopen();
                    }
                    Msg::Flush(o) => {
                        self.flush(true);
                        o.call_once(|| {});
                    }
                }
            };
        }
        if self.flush_millis > 0 {
            loop {
                match rx.recv_timeout(Duration::from_millis(self.flush_millis as u64)) {
                    Ok(msg) => {
                        process!(msg);
                        while let Ok(msg) = rx.try_recv() {
                            process!(msg);
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {
                        self.flush(false);
                    }
                    Err(RecvTimeoutError::Disconnected) => {
                        self.flush(true);
                        return;
                    }
                }
            }
        } else {
            loop {
                match rx.recv() {
                    Ok(msg) => {
                        process!(msg);
                        while let Ok(msg) = rx.try_recv() {
                            process!(msg);
                        }
                        self.flush(false);
                    }
                    Err(_) => {
                        self.flush(true);
                        return;
                    }
                }
            }
        }
    }
}
