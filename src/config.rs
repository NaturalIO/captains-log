use crate::{file_impl::LoggerSinkFile, formatter::LogFormat, log_impl::LoggerSink};
use log::{Level, LevelFilter};
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
}

pub trait SinkConfigTrait {
    fn get_level(&self) -> Level;
    fn get_file_path(&self) -> Option<Box<Path>>;
    fn build(&self) -> LoggerSink;
}

/// Config for file sink
pub struct LogFile {
    /// Directory path
    pub dir: String,

    /// max log level in this file
    pub level: Level,

    /// filename
    pub name: String,

    pub(crate) format: LogFormat,

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
