//! # captains-log
//!
//! A light-weight logger for rust, implementation base on the crate `log`.
//!
//! ## Features
//!
//! * Allow customize log format and time format.
//!
//! * Supports signal listening for log-rotate.
//!
//! * Supports multiple log files, each with its own log level.
//!
//! * Supports hook on panic.
//!
//! * On default supports multi-process/thread/coroutine log into the same file.
//! Atomic line appending can be done on Linux
//!
//! * Provides `LogFilter` for coroutine-based programs. You can set req_id in LogFilter and
//! output to log files
//!
//! ## Dependency
//!
//! ``` toml
//! [dependencies]
//! log = { version = "0.4", features = ["std", "kv_unstable"] }
//! captains_log = "0.1"
//! ```
//!
//! ## Fast setup eample:
//!
//! ```rust
//! /// #[macro_use]
//! /// extern crate captains_log;
//! /// #[macro_use]
//! /// extern crate log;
//! use log::{debug, info, error};
//! use captains_log::*;
//! use captains_log::recipe::split_error_file_logger;
//!
//! let log_builder = split_error_file_logger("/tmp", "test", log::Level::Debug);
//! setup_log(log_builder);
//!
//! // non-error msg will only appear in /tmp/test.log
//! debug!("Set a course to Sol system");
//! info!("Engage");
//!
//! // will appear in both /tmp/test.log and /tmp/test.log.wf
//! error!("Engine over heat!");
//!
//! ```
//!
//! ## Customize format example
//!
//! ``` rust
//! extern crate signal_hook;
//! extern crate chrono;
//! use captains_log::*;

//! fn format_f(r: FormatRecord) -> String {
//!     let time = r.time();
//!     let level = r.level();
//!     let file = r.file();
//!     let line = r.line();
//!     let msg = r.msg();
//!     format!("{time}|{level}|{file}:{line}|{msg}\n").to_string()
//! }
//! let debug_format = LogFormat::new(
//!     "%Y%m%d %H:%M:%S%.6f",
//!     format_f,
//! );
//! let debug_file = LogFile::new(
//!     "/tmp", "test.log", log::Level::Trace, debug_format);
//! let config = Builder::default()
//!     .signal(signal_hook::consts::SIGINT)
//!     .file(debug_file);
//!
//! setup_log(config);
//! ```



extern crate log;
extern crate captains_log_helper;
extern crate signal_hook;

#[macro_use]
extern crate enum_dispatch;

mod config;
mod time;
mod formatter;
mod file_impl;
mod log_impl;

pub mod recipe;
pub mod macros;

mod log_filter;

pub use log::{Level as LogLevel, LevelFilter as LogLevelFilter};
pub use captains_log_helper::logfn;

pub use self::{
    config::{Builder, LogFile},
    formatter::{LogFormat, FormatRecord},
    log_filter::*,
    log_impl::{setup_log},
};

#[cfg(test)]
mod tests;
