use crate::{
    config::{LogFormat, SinkConfigBuild, SinkConfigTrait},
    log_impl::{LogSink, LogSinkTrait},
    time::Timer,
};
use log::*;
use ring_file::*;

use std::hash::{Hash, Hasher};
use std::path::Path;

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
/// recipe::ring_file("/tmp/ring.log", 1024*1024, Level::Info,
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
/// The program will exit. Then you can inspect your log content on disk (for this example `/tmp/ring.log`).
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
/// latency, the buffers are put in thread local. After the program hangs, or panic, because
/// no more messages will be written to the buffer, log content can be safely copied from the buffer area to disk.
///
/// Be aware that it did not use mlock to prevent memory from being swapping. (Swapping might make the
/// code slow to prevent bug reproduction). When your memory is not enough, use a smaller buf_size and turn off the swap with `swapoff -a`.
///
/// The collected logs are from all the threads, including those exited. That means there might be
/// very old contents mixed with newer contents. We suggest you log before thread exits. And also
/// note that thread_id is reused after thread exits.
#[derive(Hash)]
pub struct LogRingFile {
    pub file_path: Option<Box<Path>>,
    pub level: Level,
    pub format: LogFormat,
    /// 0 < buf_size < i32::MAX, note this is the buffer size within each thread.
    pub buf_size: i32,
}

impl LogRingFile {
    pub fn new(
        file_path: Option<Box<Path>>, buf_size: i32, max_level: Level, format: LogFormat,
    ) -> Self {
        assert!(buf_size > 0);
        Self { buf_size, file_path, level: max_level, format }
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
        self.file_path.clone()
    }

    fn write_hash(&self, hasher: &mut Box<dyn Hasher>) {
        self.hash(hasher);
        hasher.write(b"LogRingFile");
    }
}

pub(crate) struct LogSinkRingFile {
    max_level: Level,
    formatter: LogFormat,
    ring: RingFile,
}

impl LogSinkRingFile {
    fn new(config: &LogRingFile) -> Self {
        Self {
            max_level: config.level,
            formatter: config.format.clone(),
            ring: RingFile::new(config.buf_size as usize, config.file_path.clone()),
        }
    }

    fn dump(&self) -> std::io::Result<()> {
        println!("RingFile: start dumping");
        if let Err(e) = self.ring.dump() {
            println!("RingFile: dump error {:?}", e);
            return Err(e);
        }
        println!("RingFile: dump complete");
        Ok(())
    }
}

impl LogSinkTrait for LogSinkRingFile {
    fn open(&self) -> std::io::Result<()> {
        println!("ringfile is on");
        Ok(())
    }

    fn reopen(&self) -> std::io::Result<()> {
        let _ = self.dump();
        std::process::exit(-2);
    }

    #[inline(always)]
    fn log(&self, now: &Timer, r: &Record) {
        if r.level() <= self.max_level {
            let (ts, content) = self.formatter.process_with_timestamp(now, r);
            self.ring.write(ts, content);
        }
    }

    /// Manually dump the log
    #[inline(always)]
    fn flush(&self) {
        let _ = self.dump();
    }
}
