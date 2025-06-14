use crate::log_impl::setup_log;
use crate::{
    console_impl::LoggerSinkConsole,
    file_impl::LoggerSinkFile,
    formatter::{FormatRecord, TimeFormatter},
    log_impl::LoggerSink,
    time::Timer,
};
use log::{Level, LevelFilter, Record};
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

    /// Add log-rotate signal
    pub fn signal(mut self, signal: i32) -> Self {
        self.rotation_signals.push(signal);
        self
    }

    /// Add file sink
    pub fn file(mut self, config: LogFile) -> Self {
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

    /// Setup global logger.
    /// Equals to setup_log(builder)
    pub fn build(self) -> Result<(), ()> {
        setup_log(self)
    }
}

pub trait SinkConfigTrait {
    fn get_level(&self) -> Level;
    /// Only LogFile has path
    fn get_file_path(&self) -> Option<Box<Path>>;
    fn build(&self) -> LoggerSink;
}

pub type FormatFunc = fn(FormatRecord) -> String;

#[derive(Clone)]
/// Custom formatter which adds into a log sink
pub struct LogFormat {
    time_fmt: String,
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
    /// use captains_log::{LogFile, LogFormat, FormatRecord};
    /// fn format_f(r: FormatRecord) -> String {
    ///     let time = r.time();
    ///     let level = r.level();
    ///     let msg = r.msg();
    ///     let req_id = r.key("req_id");
    ///     format!("[{time}][{level}] {msg}{req_id}\n").to_string()
    /// }
    /// let log_format = LogFormat::new("%Y-%m-%d %H:%M:%S%.6f", format_f);
    /// let log_sink = LogFile::new("/tmp", "test.log", log::Level::Info, log_format);
    /// ```

    pub fn new(time_fmt: &str, format_fn: FormatFunc) -> Self {
        Self { time_fmt: time_fmt.to_string(), format_fn }
    }

    #[inline(always)]
    pub(crate) fn process(&self, now: &Timer, record: &Record) -> String {
        let time = TimeFormatter { now, fmt_str: &self.time_fmt };
        let r = FormatRecord { record, time };
        return (self.format_fn)(r);
    }
}

/// Config for file sink
pub struct LogFile {
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

impl LogFile {
    pub fn new(dir: &str, name: &str, level: Level, format: LogFormat) -> Self {
        let file_path = Path::new(dir).join(Path::new(name)).into_boxed_path();
        Self { dir: dir.to_string(), name: name.to_string(), level, format, file_path }
    }
}

impl SinkConfigTrait for LogFile {
    fn get_level(&self) -> Level {
        self.level
    }

    fn get_file_path(&self) -> Option<Box<Path>> {
        Some(self.file_path.clone())
    }

    fn build(&self) -> LoggerSink {
        LoggerSink::File(LoggerSinkFile::new(self))
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(u8)]
pub enum ConsoleTarget {
    Stdout = 1,
    Stderr = 2,
}

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

    fn build(&self) -> LoggerSink {
        LoggerSink::Console(LoggerSinkConsole::new(self))
    }
}
