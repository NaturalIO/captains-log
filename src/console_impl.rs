use crate::{
    config::{LogFormat, SinkConfigBuild, SinkConfigTrait},
    env::EnvVarDefault,
    log_impl::{LogSink, LogSinkTrait},
    time::Timer,
};
use log::{Level, Record};
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::str::FromStr;

/// Log config for output to console
///
/// # Example
///
/// source of [crate::recipe::console_logger()]
///
/// ``` rust
/// use captains_log::*;
///
/// pub fn console_logger(target: ConsoleTarget, max_level: Level) -> Builder {
///     let console_config = LogConsole::new(target, max_level, recipe::LOG_FORMAT_DEBUG);
///     return Builder::default().add_sink(console_config);
/// }
/// ```
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

// Tried to impl blanket trait T: FromStr, rust reports conflict with
// - impl<T, U> Into<U> for T where U: From<T>;
crate::impl_from_env!(ConsoleTarget);

impl SinkConfigBuild for LogConsole {
    fn build(&self) -> LogSink {
        LogSink::Console(LogSinkConsole::new(self))
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
}

pub(crate) struct LogSinkConsole {
    target_fd: libc::c_int,
    max_level: Level,
    formatter: LogFormat,
}

impl LogSinkConsole {
    fn new(config: &LogConsole) -> Self {
        Self {
            target_fd: config.target as i32,
            max_level: config.level,
            formatter: config.format.clone(),
        }
    }
}

impl LogSinkTrait for LogSinkConsole {
    fn open(&self) -> std::io::Result<()> {
        Ok(())
    }

    fn reopen(&self) -> std::io::Result<()> {
        Ok(())
    }

    #[inline(always)]
    fn log(&self, now: &Timer, r: &Record) {
        if r.level() <= self.max_level {
            let buf = self.formatter.process(now, r);
            unsafe {
                let _ = libc::write(self.target_fd, buf.as_ptr() as *const libc::c_void, buf.len());
            }
        }
    }

    #[inline(always)]
    fn flush(&self) {}
}
