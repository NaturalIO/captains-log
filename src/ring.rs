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
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

/// The LogRingFile sink is a way to minimize the cost of logging, for debugging deadlock or race condition,
/// when the problem cannot be reproduce with ordinary log (because disk I/O will slow down the
/// execution and prevent the bug to occur).
///
/// # Usage
///
/// Enable feature `ringfile` in your Cargo.toml.
///
/// Replace the log setup with the following [recipe::ring_file()](crate::recipe::ring_file()) in your test case:
///
///(Set the level to Info or higher, to turn off other debugging logs.)
///
/// ``` rust
/// use captains_log::*;
/// // the recipe already register signal and dynamic=true, do not use test(),
/// // because test() will clear the signal.
/// recipe::ring_file("/tmp/ring.log", 512*1024*1024, Level::Info,
///     signal_consts::SIGHUP).build().expect("log setup");
/// ```
///
/// # Debugging deadlocks
///
/// Then add some high-level log to critical path in the code, try to reproduce the problem, and
/// reduce the amount of log if the bug not occur.
///
/// On start-up, it will create a limited-size ring-buffer-like memory. The log content will be held within memory but
/// not written to disk, old logs will be overwritten by new ones. Until specified signal arrives, the last
/// part of log message will be dumped to the file, in time order.
///
/// Once your program hangs up completely, find your process PID and send a signal to it.
///
/// ``` shell
/// kill -SIGHUP <pid>
/// ```
/// There will be messages printed to stdout:
///
/// ``` text
/// RingFile: start dumping
/// RingFile: dump complete
/// ```
/// Then you can inspect your log content on disk (for this example `/tmp/ring.log`).
///
/// A real-life debugging story can be found on <https://github.com/frostyplanet/crossfire-rs/issues/24>.
///
/// # Debugging assertions
///
/// When you debugging the reason of some unexpected assertions, it will automatically trigger the
/// dump in our panic hook. If you want an explicit dump, you can call:
/// ``` rust
///    log::logger().flush();
/// ```
///
/// # NOTE
///
/// The backend is provided by [RingFile crate](https://docs.rs/ring-file). To ensure low
/// latency, the buffer is protected by a spinlock instead of a mutex. After the program hangs, because
/// no more messages will be written to the buffer, log content can be safely copied from the buffer area to disk.
///
/// Be aware that it did not use mlock to prevent memory from being swapping. (Swapping might make the
/// code slow to prevent bug reproduction). When your memory is not enough, use a smaller buf_size and turn off the swap with `swapoff -a`.
///
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

#[derive(Debug, PartialEq)]
#[repr(u8)]
enum RingFileState {
    Unlock,
    Lock,
    Dump,
}

pub(crate) struct LogSinkRingFile {
    max_level: Level,
    inner: UnsafeCell<RingFile>,
    formatter: LogFormat,
    /// In order to be fast, use a spin lock instead of Mutex
    locked: AtomicU8,
}

unsafe impl Send for LogSinkRingFile {}
unsafe impl Sync for LogSinkRingFile {}

impl LogSinkRingFile {
    fn new(config: &LogRingFile) -> Self {
        Self {
            max_level: config.level,
            inner: UnsafeCell::new(RingFile::new(config.buf_size, config.file_path.to_path_buf())),
            formatter: config.format.clone(),
            locked: AtomicU8::new(RingFileState::Unlock as u8),
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

    #[inline(always)]
    fn try_lock(&self, state: RingFileState, target: RingFileState) -> Result<(), u8> {
        match self.locked.compare_exchange_weak(
            state as u8,
            target as u8,
            Ordering::Acquire,
            Ordering::Relaxed,
        ) {
            Ok(_) => Ok(()),
            Err(s) => Err(s),
        }
    }
}

impl LogSinkTrait for LogSinkRingFile {
    fn open(&self) -> std::io::Result<()> {
        println!("ringfile is on");
        Ok(())
    }

    fn reopen(&self) -> std::io::Result<()> {
        loop {
            match self.try_lock(RingFileState::Unlock, RingFileState::Dump) {
                Ok(_) => {
                    println!("RingFile: start dumping");
                    let r = self.get_inner().dump();
                    self.locked.store(RingFileState::Unlock as u8, Ordering::Release);
                    if let Err(e) = r {
                        println!("RingFile: dump error {:?}", e);
                        return Err(e);
                    } else {
                        println!("RingFile: dump complete");
                        return Ok(());
                    }
                }
                Err(s) => {
                    if s == RingFileState::Dump as u8 {
                        return Ok(());
                    }
                    std::hint::spin_loop();
                }
            }
        }
    }

    #[inline(always)]
    fn log(&self, now: &Timer, r: &Record) {
        if r.level() <= self.max_level {
            let buf = self.formatter.process(now, r);
            loop {
                match self.try_lock(RingFileState::Unlock, RingFileState::Lock) {
                    Ok(_) => {
                        let _ = self.get_inner_mut().write_all(buf.as_bytes());
                        self.locked.store(RingFileState::Unlock as u8, Ordering::Release);
                        return;
                    }
                    Err(s) => {
                        if s == RingFileState::Dump as u8 {
                            std::thread::sleep(Duration::from_millis(100));
                        } else {
                            std::hint::spin_loop();
                        }
                    }
                }
            }
        }
    }

    /// Manually dump the log
    #[inline(always)]
    fn flush(&self) {
        let _ = self.reopen();
    }
}
