//! The recipe module contains some prelude functions that construct a [Builder] for
//! convenience use. Please click to the description and source for reference.

use crate::*;
use log::Level;
use std::path;
use std::path::{Path, PathBuf};
use std::str::FromStr;

pub const DEFAULT_TIME: &'static str = "%Y-%m-%d %H:%M:%S%.6f";

/// [{time}][{level}][{file}:{line}] {msg}
pub const LOG_FORMAT_DEBUG: LogFormat = LogFormat::new(DEFAULT_TIME, debug_format_f);

/// [{time}][{level}] {msg}
pub const LOG_FORMAT_PROD: LogFormat = LogFormat::new(DEFAULT_TIME, prod_format_f);

/// formatter function: [{time}][{level}][{file}:{line}] {msg}
pub fn debug_format_f(r: FormatRecord) -> String {
    let time = r.time();
    let level = r.level();
    let file = r.file();
    let line = r.line();
    let msg = r.msg();
    format!("[{time}][{level}][{file}:{line}] {msg}\n").to_string()
}

/// formatter function: [{time}][{level}] {msg}
pub fn prod_format_f(r: FormatRecord) -> String {
    let time = r.time();
    let level = r.level();
    let msg = r.msg();
    format!("[{time}][{level}] {msg}\n").to_string()
}

fn console_logger(target: ConsoleTarget, max_level: Level) -> Builder {
    let console_config = LogConsole::new(target, max_level, LOG_FORMAT_DEBUG);
    let mut config = Builder::default().console(console_config);
    // panic on debugging
    #[cfg(debug_assertions)]
    {
        config.continue_when_panic = false;
    }
    // do not panic on release
    #[cfg(not(debug_assertions))]
    {
        config.continue_when_panic = true;
    }
    return config;
}

/// Output to stdout with LOG_FORMAT_DEBUG, with dynamic=true.
///
/// You don't care the speed when output to console.
#[inline]
pub fn stdout_logger(max_level: Level) -> Builder {
    console_logger(ConsoleTarget::Stdout, max_level).test()
}

/// Output to stderr with LOG_FORMAT_DEBUG, with dynamic=true.
///
/// You don't care the speed when output to console.
#[inline]
pub fn stderr_logger(max_level: Level) -> Builder {
    console_logger(ConsoleTarget::Stderr, max_level).test()
}

/// Configure dynamic file/console logger from environment.
///
/// # Arguments:
///
///   - file_env_name:
///
///     If valid as stdout/stderr/1/2, output to console target;
///
///     When a file path is configured, create a raw_file_logger();
///
///     For empty string, default output to Stderr.
///
///   - level_env_name: configure the log level, default to Info.
///
/// # Example:
///
/// ``` rust
/// use captains_log::recipe;
/// let _ = recipe::env_logger("LOG_FILE", "LOG_LEVEL").build();
/// ```
pub fn env_logger(file_env_name: &str, level_env_name: &str) -> Builder {
    let level: Level = env_or(level_env_name, Level::Info).into();
    let mut console: Option<ConsoleTarget> = None;
    if let Ok(file_path) = std::env::var(file_env_name) {
        if let Ok(target) = ConsoleTarget::from_str(file_path.as_str()) {
            console = Some(target);
        } else if file_path.len() > 0 {
            return raw_file_logger(file_path, level).test();
        }
    }
    return console_logger(console.unwrap_or(ConsoleTarget::Stderr), level).test();
}

/// Setup one log file, with custom time_fmt & format_func.
///
/// See the source for details.
///
/// The type of file_path can be &str / String / &OsStr / OsString / Path / PathBuf
pub fn raw_file_logger_custom<P: Into<PathBuf>>(
    file_path: P, max_level: Level, time_fmt: &'static str, format_func: FormatFunc,
) -> Builder {
    let format = LogFormat::new(time_fmt, format_func);
    let _file_path = file_path.into();
    let p = path::absolute(&_file_path).expect("path convert to absolute");
    let dir = p.parent().unwrap();
    let file_name = Path::new(p.file_name().unwrap());
    let file = LogRawFile::new(dir, file_name, max_level, format);
    let mut config = Builder::default().signal(signal_hook::consts::SIGUSR1).raw_file(file);
    // panic on debugging
    #[cfg(debug_assertions)]
    {
        config.continue_when_panic = false;
    }
    // do not panic on release
    #[cfg(not(debug_assertions))]
    {
        config.continue_when_panic = true;
    }
    return config;
}

/// Setup one log file.
///
/// See the source for details.
///
/// The type of file_path can be &str / String / &OsStr / OsString / Path / PathBuf
pub fn raw_file_logger<P: Into<PathBuf>>(file_path: P, max_level: Level) -> Builder {
    raw_file_logger_custom(file_path, max_level, DEFAULT_TIME, debug_format_f)
}

/// Setup two log files.
/// One as "{{name}}.log" for debug purpose, with file line to track problem.
/// One as "{{name}}.log.wf" for error level log.
/// See the source for details.
///
/// The type of `dir` can be &str / String / &OsStr / OsString / Path / PathBuf.
///
/// The type of `name` can be &str / String.
pub fn split_error_file_logger<P1, P2>(dir: P1, name: P2, max_level: Level) -> Builder
where
    P1: Into<PathBuf>,
    P2: Into<String>,
{
    let _name: String = name.into();
    let debug_file_name = format!("{}.log", _name);
    let _dir: PathBuf = dir.into();
    let debug_file = LogRawFile::new(_dir.clone(), debug_file_name, max_level, LOG_FORMAT_DEBUG);
    let err_file_name = format!("{}.log.wf", _name);
    let error_file = LogRawFile::new(_dir.clone(), err_file_name, Level::Error, LOG_FORMAT_PROD);

    let mut config = Builder::default()
        .signal(signal_hook::consts::SIGUSR1)
        .raw_file(debug_file)
        .raw_file(error_file);

    // panic on debugging
    #[cfg(debug_assertions)]
    {
        config.continue_when_panic = false;
    }
    // do not panic on release
    #[cfg(not(debug_assertions))]
    {
        config.continue_when_panic = true;
    }
    return config;
}

/// Setup one buffered log file, with custom time_fmt & format_func.
///
/// See the source for details.
///
/// The type of file_path can be &str / String / &OsStr / OsString / Path / PathBuf
///
/// flush_millis: default to 0, means always flush when no more message to write. when larger than
/// zero, will wait for new message when timeout occur.
/// the max value is 1000 (1 sec).
pub fn buffered_file_logger_custom<P: Into<PathBuf>>(
    file_path: P, max_level: Level, time_fmt: &'static str, format_func: FormatFunc,
    flush_millis: usize, rotate: Option<Rotation>,
) -> Builder {
    let format = LogFormat::new(time_fmt, format_func);
    let _file_path = file_path.into();
    let p = path::absolute(&_file_path).expect("path convert to absolute");
    let dir = p.parent().unwrap();
    let file_name = Path::new(p.file_name().unwrap());
    let file = LogBufFile::new(dir, file_name, max_level, format, flush_millis);
    let mut config = Builder::default().signal(signal_hook::consts::SIGUSR1).buf_file(file);
    // panic on debugging
    #[cfg(debug_assertions)]
    {
        config.continue_when_panic = false;
    }
    // do not panic on release
    #[cfg(not(debug_assertions))]
    {
        config.continue_when_panic = true;
    }
    return config;
}

pub fn buffered_file_logger<P: Into<PathBuf>>(file_path: P, max_level: Level) -> Builder {
    buffered_file_logger_custom(file_path, max_level, DEFAULT_TIME, debug_format_f, 0, None)
}

pub fn buffered_rotated_file_logger<P: Into<PathBuf>>(
    file_path: P, max_level: Level, rotation: Rotation,
) -> Builder {
    buffered_file_logger_custom(
        file_path,
        max_level,
        DEFAULT_TIME,
        debug_format_f,
        0,
        Some(rotation),
    )
}
