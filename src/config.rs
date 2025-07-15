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
use std::path::{Path, PathBuf};
use std::str::FromStr;

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

    fn build(&self) -> LoggerSink {
        LoggerSink::File(LoggerSinkFile::new(self))
    }
}

#[derive(Copy, Clone, Debug, Hash, PartialEq)]
#[repr(u8)]
pub enum ConsoleTarget {
    Stdout = 1,
    Stderr = 2,
}

impl FromStr for ConsoleTarget {
    type Err = ();

    /// accepts case-insensitive: stdout, stderr, out, err, 1, 2
    fn from_str(s: &str) -> Result<Self, ()> {
        let v = s.to_lowercase();
        match v.as_str() {
            "stdout" => Ok(ConsoleTarget::Stdout),
            "stderr" => Ok(ConsoleTarget::Stderr),
            "out" => Ok(ConsoleTarget::Stdout),
            "err" => Ok(ConsoleTarget::Stderr),
            "1" => Ok(ConsoleTarget::Stdout),
            "2" => Ok(ConsoleTarget::Stderr),
            _ => Err(()),
        }
    }
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

pub struct EnvVarDefault<'a, T> {
    name: &'a str,
    default: T,
}

/// To config some logger setting with env.
///
/// Read value from environment, and set with default if not exists.
///
/// NOTE: the arguments to load from env_or() must support owned values.
///
/// Example:
///
/// ```rust
/// use captains_log::*;
/// let _level: log::Level = env_or("LOG_LEVEL", Level::Info).into();
/// let _file_path: String = env_or("LOG_FILE", "/tmp/test.log").into();
/// let _console: ConsoleTarget = env_or("LOG_CONSOLE", ConsoleTarget::Stdout).into();
/// ```
pub fn env_or<'a, T>(name: &'a str, default: T) -> EnvVarDefault<'a, T> {
    EnvVarDefault { name, default }
}

impl<'a> Into<String> for EnvVarDefault<'a, &'a str> {
    fn into(self) -> String {
        if let Ok(v) = std::env::var(&self.name) {
            return v;
        }
        return self.default.to_string();
    }
}

impl<'a, P: AsRef<Path>> Into<PathBuf> for EnvVarDefault<'a, P> {
    fn into(self) -> PathBuf {
        if let Some(v) = std::env::var_os(&self.name) {
            if v.len() > 0 {
                return PathBuf::from(v);
            }
        }
        return self.default.as_ref().to_path_buf();
    }
}

macro_rules! impl_from_env {
    ($type: tt) => {
        impl<'a> Into<$type> for EnvVarDefault<'a, $type> {
            #[inline]
            fn into(self) -> $type {
                if let Ok(v) = std::env::var(&self.name) {
                    match $type::from_str(&v) {
                        Ok(r) => return r,
                        Err(_) => {
                            eprintln!(
                                "env {}={} is not valid, set to {:?}",
                                self.name, v, self.default
                            );
                        }
                    }
                }
                return self.default;
            }
        }
    };
}

// Tried to impl blanket trait T: FromStr, rust reports conflict with
// - impl<T, U> Into<U> for T where U: From<T>;
impl_from_env!(ConsoleTarget);
impl_from_env!(Level);

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

    #[test]
    fn test_env_config() {
        // test log level
        unsafe { std::env::set_var("LEVEL", "warn") };
        let level: Level = env_or("LEVEL", Level::Debug).into();
        assert_eq!(level, Level::Warn);
        unsafe { std::env::set_var("LEVEL", "WARN") };
        let level: Level = env_or("LEVEL", Level::Debug).into();
        assert_eq!(level, Level::Warn);

        assert_eq!(ConsoleTarget::from_str("Stdout").unwrap(), ConsoleTarget::Stdout);
        assert_eq!(ConsoleTarget::from_str("StdERR").unwrap(), ConsoleTarget::Stderr);
        assert_eq!(ConsoleTarget::from_str("1").unwrap(), ConsoleTarget::Stdout);
        assert_eq!(ConsoleTarget::from_str("2").unwrap(), ConsoleTarget::Stderr);
        assert_eq!(ConsoleTarget::from_str("0").unwrap_err(), ());

        // test console target
        unsafe { std::env::set_var("CONSOLE", "stderr") };
        let target: ConsoleTarget = env_or("CONSOLE", ConsoleTarget::Stdout).into();
        assert_eq!(target, ConsoleTarget::Stderr);
        unsafe { std::env::set_var("CONSOLE", "") };
        let target: ConsoleTarget = env_or("CONSOLE", ConsoleTarget::Stdout).into();
        assert_eq!(target, ConsoleTarget::Stdout);

        // test path
        unsafe { std::env::set_var("LOG_PATH", "/tmp/test.log") };
        let path: PathBuf = env_or("LOG_PATH", "/tmp/other.log").into();
        assert_eq!(path, Path::new("/tmp/test.log").to_path_buf());

        unsafe { std::env::set_var("LOG_PATH", "") };
        let path: PathBuf = env_or("LOG_PATH", "/tmp/other.log").into();
        assert_eq!(path, Path::new("/tmp/other.log").to_path_buf());

        let _builder = recipe::raw_file_logger(env_or("LOG_PATH", "/tmp/other.log"), Level::Info);
        let _builder =
            recipe::raw_file_logger(env_or("LOG_PATH", "/tmp/other.log".to_string()), Level::Info);
    }
}
