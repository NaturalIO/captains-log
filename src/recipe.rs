use crate::{
    config::{Builder, ConsoleTarget, FormatFunc, LogConsole, LogFormat, LogRawFile},
    formatter::FormatRecord,
};
use log::Level;

pub const DEFAULT_TIME: &'static str = "%Y-%m-%d %H:%M:%S%.6f";

pub fn debug_format_f(r: FormatRecord) -> String {
    let time = r.time();
    let level = r.level();
    let file = r.file();
    let line = r.line();
    let msg = r.msg();
    format!("[{time}][{level}][{file}:{line}] {msg}\n").to_string()
}

pub fn error_format_f(r: FormatRecord) -> String {
    let time = r.time();
    let level = r.level();
    let msg = r.msg();
    format!("[{time}][{level}] {msg}\n").to_string()
}

fn console_logger(target: ConsoleTarget, max_level: Level) -> Builder {
    let debug_format = LogFormat::new(DEFAULT_TIME, debug_format_f);
    let console_config = LogConsole::new(target, max_level, debug_format);
    let mut config = Builder::default().console(console_config);
    // panic on debuging
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

#[inline]
pub fn stdout_logger(max_level: Level) -> Builder {
    console_logger(ConsoleTarget::Stdout, max_level)
}

/// Output to stdout, with dynamic=true for test cases.
pub fn stdout_test_logger(max_level: Level) -> Builder {
    let mut builder = stdout_logger(max_level);
    builder.dynamic = true;
    builder
}

#[inline]
pub fn stderr_logger(max_level: Level) -> Builder {
    console_logger(ConsoleTarget::Stderr, max_level)
}

/// Output to stderr, with dynamic=true for test cases.
#[inline]
pub fn stderr_test_logger(max_level: Level) -> Builder {
    let mut builder = stderr_logger(max_level);
    builder.dynamic = true;
    builder
}

/// In this funtion, setup one log file, with custom time_fmt & format_func.
/// See the source for details.
pub fn raw_file_logger_custom(
    dir: &str, name: &str, max_level: Level, time_fmt: &str, format_func: FormatFunc,
) -> Builder {
    let debug_format = LogFormat::new(time_fmt, format_func);
    let debug_file =
        LogRawFile::new(dir, &format!("{}.log", name).to_string(), max_level, debug_format);
    let mut config = Builder::default().signal(signal_hook::consts::SIGUSR1).raw_file(debug_file);
    // panic on debuging
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

/// In this funtion, setup one log file.
/// See the source for details.
pub fn raw_file_logger(dir: &str, name: &str, max_level: Level) -> Builder {
    raw_file_logger_custom(dir, name, max_level, DEFAULT_TIME, debug_format_f)
}

/// In this funtion, setup two log files.
/// One for debug purpose, with code file line to track problem.
/// One for error log.
/// See the source for details.
pub fn split_error_file_logger(dir: &str, name: &str, max_level: Level) -> Builder {
    let debug_format = LogFormat::new(DEFAULT_TIME, debug_format_f);

    let err_format = LogFormat::new(DEFAULT_TIME, error_format_f);
    let debug_file =
        LogRawFile::new(dir, &format!("{}.log", name).to_string(), max_level, debug_format);
    let error_file =
        LogRawFile::new(dir, &format!("{}.log.wf", name).to_string(), Level::Error, err_format);

    let mut config = Builder::default()
        .signal(signal_hook::consts::SIGUSR1)
        .raw_file(debug_file)
        .raw_file(error_file);

    // panic on debuging
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
