use crate::{
    config::{LogFormat, SinkConfigBuild, SinkConfigTrait},
    log_impl::{LogSink, LogSinkTrait},
    time::Timer,
};
use log::*;
use ring_file::*;

use std::cell::UnsafeCell;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::mem::transmute;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// The LogRingFile sink is a tool for debugging deadlock or race condition,
/// when the problem cannot be reproduce with ordinary log (because disk I/O will slow down the
/// execution and prevent the bug to occur).
///
/// # Usage
///
/// Enable feature `ringfile` in your Cargo.toml.
///
/// Replace the log setup with the following in your test case:
/// ``` rust
/// use captains_log::*;
/// recipe::ring_file("/tmp/ring.log", 512*1024*1024, Level::Info,
///     signal_consts::SIGHUP).test().build().expect("log setup");
/// ```
///
/// Then add some high-level log to critical path in the code, try to reproduce the problem, and
/// reduce the amount of log if the bug not occur.
///
/// On start-up, it will create a limited-size ring-buffer-like memory. The log content will be hold within memory but
/// not written to disk, old logs will be overwritten by new ones. Until specified signal arrives, the last
/// part of log message will be dumped to file, in time order.
///
/// Once your program hangs up completely , find your process pid and send a signal to it.
///
/// ``` shell
/// kill -SIGHUP <pid>
/// ```
/// There will be messages print to stdout:
///
/// ``` text
/// RingFile: start dumping
/// RingFile: dump complete
/// ```
///
/// Then you can inspect your log content on disk (for this example `/tmp/ring.log`).
///
/// The backend is provided by [RingFile crate](https://docs.rs/ring-file). To ensure low
/// latency, the buffer is protected by a spinlock instead of a mutex. After the program hangs, because
/// no more message will be written to the buffer, log content can be safely copied from the buffer area to disk.
///
/// A real-life debugging story can be found on <https://github.com/frostyplanet/crossfire-rs/issues/24>.
#[derive(Hash)]
pub struct LogRingFile {
    pub file_path: Box<Path>,
    pub level: Level,
    pub format: LogFormat,
    /// 0 < buf_size < i32::MAX
    pub buf_size: i32,
}

impl LogRingFile {
    pub fn new<P: Into<PathBuf>>(
        file_path: P, buf_size: i32, max_level: Level, format: LogFormat,
    ) -> Self {
        assert!(buf_size > 0);
        Self { buf_size, file_path: file_path.into().into_boxed_path(), level: max_level, format }
    }
}

impl SinkConfigBuild for LogRingFile {
    fn build(&self) -> LogSink {
        LogSink::RingFile(LogSinkRingFile::new(self))
    }
}

impl SinkConfigTrait for LogRingFile {
    fn get_level(&self) -> Level {
        self.level
    }

    fn get_file_path(&self) -> Option<Box<Path>> {
        Some(self.file_path.clone())
    }

    fn write_hash(&self, hasher: &mut Box<dyn Hasher>) {
        self.hash(hasher);
        hasher.write(b"LogRingFile");
    }
}

pub(crate) struct LogSinkRingFile {
    max_level: Level,
    inner: UnsafeCell<RingFile>,
    formatter: LogFormat,
    /// In order to be fast, use a spin lock instead of Mutex
    locked: AtomicBool,
}

unsafe impl Send for LogSinkRingFile {}
unsafe impl Sync for LogSinkRingFile {}

impl LogSinkRingFile {
    fn new(config: &LogRingFile) -> Self {
        Self {
            max_level: config.level,
            inner: UnsafeCell::new(RingFile::new(config.buf_size, config.file_path.to_path_buf())),
            formatter: config.format.clone(),
            locked: AtomicBool::new(false),
        }
    }

    #[inline(always)]
    fn get_inner(&self) -> &RingFile {
        unsafe { transmute(self.inner.get()) }
    }

    #[inline(always)]
    fn get_inner_mut(&self) -> &mut RingFile {
        unsafe { transmute(self.inner.get()) }
    }
}

impl LogSinkTrait for LogSinkRingFile {
    fn open(&self) -> std::io::Result<()> {
        Ok(())
    }

    fn reopen(&self) -> std::io::Result<()> {
        println!("RingFile: start dumping");
        if let Err(e) = self.get_inner().dump() {
            println!("RingFile: dump error {:?}", e);
            Err(e)
        } else {
            println!("RingFile: dump complete");
            Ok(())
        }
    }

    #[inline(always)]
    fn log(&self, now: &Timer, r: &Record) {
        if r.level() <= self.max_level {
            let buf = self.formatter.process(now, r);
            while self
                .locked
                .compare_exchange_weak(false, true, Ordering::SeqCst, Ordering::Relaxed)
                .is_err()
            {
                std::hint::spin_loop();
            }
            let _ = self.get_inner_mut().write_all(buf.as_bytes());
            self.locked.store(false, Ordering::Release);
        }
    }

    #[inline(always)]
    fn flush(&self) {}
}
