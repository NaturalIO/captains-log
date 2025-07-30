use crate::buf_file_impl::LogBufFile;
use crate::console_impl::LogConsole;
use crate::file_impl::LogRawFile;
use crate::log_impl::setup_log;
use crate::{
    formatter::{FormatRecord, TimeFormatter},
    log_impl::LogSink,
    time::Timer,
};
use log::{Level, LevelFilter, Record};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;

/// Global config to setup logger
/// See crate::recipe for usage
#[derive(Default)]
pub struct Builder {
    /// When dynamic==true,
    ///   Can safely re-initialize GlobalLogger even it exists,
    ///   useful to setup different types of logger in test suits.
    /// When dynamic==false,
    ///   Only initialize once, logger sinks setting cannot be change afterwards.
    ///   More efficient for production environment.
    pub dynamic: bool,

    /// Listen for signal of log-rotate
    /// NOTE: Once logger started to listen signal, does not support dynamic reconfigure.
    pub rotation_signals: Vec<i32>,

    /// Hookup to log error when panic
    pub panic: bool,

    /// Whether to exit program after panic
    pub continue_when_panic: bool,

    /// Different types of log sink
    pub(crate) sinks: Vec<Box<dyn SinkConfigTrait>>,
}

impl Builder {
    pub fn new() -> Self {
        Self::default()
    }

    /// For test cases, set dynamic=true and turn Off signal.
    /// Call this with pre-set recipe for convenient.
    pub fn test(mut self) -> Self {
        self.dynamic = true;
        self.rotation_signals.clear();
        self
    }

    /// Add log-rotate signal
    pub fn signal(mut self, signal: i32) -> Self {
        self.rotation_signals.push(signal);
        self
    }

    /// Add raw file sink that supports multiprocess atomic append
    pub fn raw_file(mut self, config: LogRawFile) -> Self {
        self.sinks.push(Box::new(config));
        self
    }

    /// Add buffered file sink which merged I/O and delay flush
    pub fn buf_file(mut self, config: LogBufFile) -> Self {
        self.sinks.push(Box::new(config));
        self
    }

    /// Add console sink
    pub fn console(mut self, config: LogConsole) -> Self {
        self.sinks.push(Box::new(config));
        self
    }

    #[cfg(feature = "syslog")]
    /// Add syslog sink
    pub fn syslog(mut self, config: crate::Syslog) -> Self {
        self.sinks.push(Box::new(config));
        self
    }

    /// Return the max log level in the log sinks
    pub fn get_max_level(&self) -> LevelFilter {
        let mut max_level = Level::Error;
        for sink in &self.sinks {
            let level = sink.get_level();
            if level > max_level {
                max_level = level;
            }
        }
        return max_level.to_level_filter();
    }

    /// Calculate checksum of the setting for init() comparison
    pub(crate) fn cal_checksum(&self) -> u64 {
        let mut hasher = Box::new(DefaultHasher::new()) as Box<dyn Hasher>;
        self.dynamic.hash(&mut hasher);
        self.rotation_signals.hash(&mut hasher);
        self.panic.hash(&mut hasher);
        self.continue_when_panic.hash(&mut hasher);
        for sink in &self.sinks {
            sink.write_hash(&mut hasher);
        }
        hasher.finish()
    }

    /// Setup global logger.
    /// Equals to setup_log(builder)
    pub fn build(self) -> Result<(), ()> {
        setup_log(self)
    }
}

pub(crate) trait SinkConfigTrait {
    /// get max log level of the sink
    fn get_level(&self) -> Level;
    /// Only file sink has path
    #[allow(dead_code)]
    fn get_file_path(&self) -> Option<Box<Path>>;
    /// Calculate hash for config comparison
    fn write_hash(&self, hasher: &mut Box<dyn Hasher>);
    /// Build an actual sink from config
    fn build(&self) -> LogSink;
}

pub type FormatFunc = fn(FormatRecord) -> String;

/// Custom formatter which adds into a log sink
#[derive(Clone, Hash)]
pub struct LogFormat {
    time_fmt: &'static str,
    format_fn: FormatFunc,
}

impl LogFormat {
    /// # Arguments
    ///
    /// time_fmt: refer to chrono::format::strftime.
    ///
    /// format_fn:
    /// Since std::fmt only support compile time format,
    /// you have to write a static function to format the log line
    ///
    /// # Example
    /// ```
    /// use captains_log::{LogRawFile, LogFormat, FormatRecord};
    /// fn format_f(r: FormatRecord) -> String {
    ///     let time = r.time();
    ///     let level = r.level();
    ///     let msg = r.msg();
    ///     let req_id = r.key("req_id");
    ///     format!("[{time}][{level}] {msg}{req_id}\n").to_string()
    /// }
    /// let log_format = LogFormat::new("%Y-%m-%d %H:%M:%S%.6f", format_f);
    /// let log_sink = LogRawFile::new("/tmp", "test.log", log::Level::Info, log_format);
    /// ```

    pub const fn new(time_fmt: &'static str, format_fn: FormatFunc) -> Self {
        Self { time_fmt, format_fn }
    }

    #[inline(always)]
    pub(crate) fn process(&self, now: &Timer, record: &Record) -> String {
        let time = TimeFormatter { now, fmt_str: self.time_fmt };
        let r = FormatRecord { record, time };
        return (self.format_fn)(r);
    }
}
