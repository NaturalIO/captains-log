use crate::log_impl::setup_log;
use crate::{
    console_impl::LoggerSinkConsole,
    file_impl::LoggerSinkFile,
    formatter::{FormatRecord, TimeFormatter},
    log_impl::LoggerSink,
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
    pub sinks: Vec<Box<dyn SinkConfigTrait>>,
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

    /// Add console sink
    pub fn console(mut self, config: LogConsole) -> Self {
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

pub trait SinkConfigTrait {
    /// get max log level of the sink
    fn get_level(&self) -> Level;
    /// Only file sink has path
    fn get_file_path(&self) -> Option<Box<Path>>;
    /// Calculate hash for config comparison
    fn write_hash(&self, hasher: &mut Box<dyn Hasher>);
    /// Build an actual sink from config
    fn build(&self) -> LoggerSink;
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

/// Config for file sink that supports atomic append from multiprocess.
/// For log rotation, you need system log-rotate service to notify with signal.
#[derive(Hash)]
pub struct LogRawFile {
    /// Directory path
    pub dir: String,

    /// max log level in this file
    pub level: Level,

    /// filename
    pub name: String,

    pub format: LogFormat,

    /// path: dir/name
    pub file_path: Box<Path>,
}

impl LogRawFile {
    pub fn new(dir: &str, name: &str, level: Level, format: LogFormat) -> Self {
        let file_path = Path::new(dir).join(Path::new(name)).into_boxed_path();
        Self { dir: dir.to_string(), name: name.to_string(), level, format, file_path }
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

    fn build(&self) -> LoggerSink {
        LoggerSink::File(LoggerSinkFile::new(self))
    }
}

#[derive(Copy, Clone, Debug, Hash)]
#[repr(u8)]
pub enum ConsoleTarget {
    Stdout = 1,
    Stderr = 2,
}

#[derive(Hash)]
pub struct LogConsole {
    pub target: ConsoleTarget,

    /// max log level in this file
    pub level: Level,

    pub format: LogFormat,
}

impl LogConsole {
    pub fn new(target: ConsoleTarget, level: Level, format: LogFormat) -> Self {
        Self { target, level, format }
    }
}

impl SinkConfigTrait for LogConsole {
    fn get_level(&self) -> Level {
        self.level
    }

    fn get_file_path(&self) -> Option<Box<Path>> {
        None
    }

    fn write_hash(&self, hasher: &mut Box<dyn Hasher>) {
        self.hash(hasher);
        hasher.write(b"LogConsole");
    }

    fn build(&self) -> LoggerSink {
        LoggerSink::Console(LoggerSinkConsole::new(self))
    }
}
