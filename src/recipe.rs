use crate::{
    config::{Builder, ConsoleTarget, LogConsole, LogFile},
    formatter::{FormatRecord, LogFormat},
};
use log::Level;

pub const DEFAULT_TIME: &'static str = "%Y-%m-%d %H:%M:%S%.6f";

fn debug_format_f(r: FormatRecord) -> String {
    let time = r.time();
    let level = r.level();
    let file = r.file();
    let line = r.line();
    let msg = r.msg();
    let req_id = r.key("req_id");
    format!("[{time}][{level}][{file}:{line}] {msg}{req_id}\n").to_string()
}

fn error_format_f(r: FormatRecord) -> String {
    let time = r.time();
    let level = r.level();
    let msg = r.msg();
    let req_id = r.key("req_id");
    format!("[{time}][{level}] {msg}{req_id}\n").to_string()
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

#[inline]
pub fn stderr_logger(max_level: Level) -> Builder {
    console_logger(ConsoleTarget::Stderr, max_level)
}

/// In this funtion, setup two log files.
/// One for debug purpose, with code file line to track problem
/// One for error log.
/// See the source for details.
pub fn split_error_file_logger(dir: &str, name: &str, max_level: Level) -> Builder {
    let debug_format = LogFormat::new(DEFAULT_TIME, debug_format_f);

    let err_format = LogFormat::new(DEFAULT_TIME, error_format_f);
    let debug_file =
        LogFile::new(dir, &format!("{}.log", name).to_string(), max_level, debug_format);
    let error_file =
        LogFile::new(dir, &format!("{}.log.wf", name).to_string(), Level::Error, err_format);

    let mut config =
        Builder::default().signal(signal_hook::consts::SIGUSR1).file(debug_file).file(error_file);

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
