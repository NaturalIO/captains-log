use crate::{
    config::{Builder, ConsoleTarget, FormatFunc, LogConsole, LogFormat, LogRawFile},
    formatter::FormatRecord,
};
use log::Level;

pub const DEFAULT_TIME: &'static str = "%Y-%m-%d %H:%M:%S%.6f";

pub const LOG_FORMAT_DEBUG: LogFormat = LogFormat::new(DEFAULT_TIME, debug_format_f);

pub const LOG_FORMAT_PROD: LogFormat = LogFormat::new(DEFAULT_TIME, prod_format_f);

pub fn debug_format_f(r: FormatRecord) -> String {
    let time = r.time();
    let level = r.level();
    let file = r.file();
    let line = r.line();
    let msg = r.msg();
    format!("[{time}][{level}][{file}:{line}] {msg}\n").to_string()
}

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

#[inline]
pub fn stdout_logger(max_level: Level) -> Builder {
    console_logger(ConsoleTarget::Stdout, max_level)
}

/// Output to stdout, with dynamic=true for test cases.
pub fn stdout_test_logger(max_level: Level) -> Builder {
    stdout_logger(max_level).test()
}

#[inline]
pub fn stderr_logger(max_level: Level) -> Builder {
    console_logger(ConsoleTarget::Stderr, max_level)
}

/// Output to stderr, with dynamic=true for test cases.
#[inline]
pub fn stderr_test_logger(max_level: Level) -> Builder {
    stderr_logger(max_level).test()
}

/// Setup one log file, with custom time_fmt & format_func.
/// See the source for details.
pub fn raw_file_logger_custom(
    dir: &str, name: &str, max_level: Level, time_fmt: &'static str, format_func: FormatFunc,
) -> Builder {
    let format = LogFormat::new(time_fmt, format_func);
    let file = LogRawFile::new(dir, &format!("{}.log", name).to_string(), max_level, format);
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
/// See the source for details.
pub fn raw_file_logger(dir: &str, name: &str, max_level: Level) -> Builder {
    raw_file_logger_custom(dir, name, max_level, DEFAULT_TIME, debug_format_f)
}

/// Setup two log files.
/// One for debug purpose, with code file line to track problem.
/// One for error log.
/// See the source for details.
pub fn split_error_file_logger(dir: &str, name: &str, max_level: Level) -> Builder {
    let debug_file =
        LogRawFile::new(dir, &format!("{}.log", name).to_string(), max_level, LOG_FORMAT_DEBUG);
    let error_file = LogRawFile::new(
        dir,
        &format!("{}.log.wf", name).to_string(),
        Level::Error,
        LOG_FORMAT_PROD,
    );

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
