//! # captains-log
//!
//! A light-weight logger for rust, implementation base on the crate `log`.
//!
//! ## Features
//!
//! * Allow customize log format and time format. Refer to [LogFormat].
//!
//! * Supports multiple types of sink stacking, each with its own log level.
//!
//!     + [Builder]::console([LogConsole]):   Console output to stdout/stderr.
//!
//!     + [Builder]::raw_file([LogRawFile]):  Support atomic appending from multi-process on linux
//!
//! * Log panic message by default.
//!
//! * Supports signal listening for log-rotate. Refer to [Builder]::signal()
//!
//! * Fine-grain module-level control.
//!
//!   Provides [LogFilter] to filter specified logs on-the-fly.
//!
//! * API-level log handling.
//!
//!   Provides [LogFilterKV] for API logging with additional key.
//!
//!   For example, you can set `req_id` in `LogFilterKV`, and track the
//! complete request handling procedure from log.
//!
//! * For test suits usage:
//!
//!   Allow dynamic reconfigure logger setting in different test function.
//!
//!   (NOTE: currently signal_listener does not support reconfigure).
//!
//!   Provides an attribute macro [#\[logfn\]] to wrap test function.
//!  Logging test-start and test-end.
//!
//! * Provides a [parser] to work on your log files.
//!
//! ## Usage
//!
//! Cargo.toml
//!
//! ``` toml
//! [dependencies]
//! log = { version = "0.4", features = ["std", "kv_unstable"] }
//! captains_log = "0.4"
//! ```
//!
//! lib.rs or main.rs:
//! ```
//! #[macro_use]
//! extern crate captains_log;
//! #[macro_use]
//! extern crate log;
//! ```
//!
//! ## Production example:
//!
//! <font color=Blue>Refer to recipe in `recipe` module, including console & file output. </font>
//!
//! ```rust

//! use log::{debug, info, error};
//! use captains_log::recipe::split_error_file_logger;
//!
//! // You'll get /tmp/test.log with all logs, and /tmp/test.log.wf only with error logs.
//! let mut log_builder = split_error_file_logger("/tmp", "test", log::Level::Debug);
//! // Builder::build() is equivalent of setup_log().
//! log_builder.build();
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
//! ## Unit test example
//!
//! To setup different log config on different tests.
//!
//! call <font color=Blue> test() </font> on [Builder],
//! which enable dynamic log config and disable signal_hook.
//!
//! ```rust
//! use log::{debug, info, error, Level};
//! use captains_log::recipe;
//!
//! #[test]
//! fn test1() {
//!     recipe::raw_file_logger(
//!         "/tmp", "test1", Level::Debug).test().build();
//!     info!("doing test1");
//! }
//!
//! #[test]
//! fn test2() {
//!     recipe::raw_file_logger(
//!         "/tmp", "test2", Level::Debug).test().build();
//!     info!("doing test2");
//! }
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
//! let debug_file = LogRawFile::new(
//!     "/tmp", "test.log", log::Level::Trace, debug_format);
//! let config = Builder::default()
//!     .signal(signal_hook::consts::SIGINT)
//!     .raw_file(debug_file);
//!
//! config.build();
//! ```
//!
//! ## Fine-grain module-level control
//!
//! Place `LogFilter` in Arc and share among coroutines.
//! Log level can be changed on-the-fly.
//!
//! ``` rust
//! use std::sync::Arc;
//! use captains_log::*;
//! log::set_max_level(log::LevelFilter::Debug);
//! let logger_io = Arc::new(LogFilter::new());
//! let logger_req = Arc::new(LogFilter::new());
//! logger_io.set_level(log::Level::Error);
//! logger_req.set_level(log::Level::Debug);
//! logger_debug!(logger_req, "Begin handle req ...");
//! logger_debug!(logger_io, "Issue io to disk ...");
//! logger_error!(logger_req, "Req invalid ...");
//!
//! ```
//!
//! ## API-level log handling
//!
//! Request log can be track by custom key `req_id`, which kept in `LogFilterKV`.
//!
//! ``` rust
//! use captains_log::*;
//! use log::*;
//! fn debug_format_req_id_f(r: FormatRecord) -> String {
//!     let time = r.time();
//!     let level = r.level();
//!     let file = r.file();
//!     let line = r.line();
//!     let msg = r.msg();
//!     let req_id = r.key("req_id");
//!     format!("[{time}][{level}][{file}:{line}] {msg}{req_id}\n").to_string()
//! }
//! let builder = recipe::raw_file_logger_custom("/tmp", "log_filter.log", log::Level::Debug,
//!     recipe::DEFAULT_TIME, debug_format_req_id_f);
//! builder.build().expect("setup_log");
//! let logger = LogFilterKV::new("req_id", format!("{:016x}", 123).to_string());
//! info!("API service started");
//! logger_debug!(logger, "Req / received");
//! logger_debug!(logger, "header xxx");
//! logger_info!(logger, "Req / 200 complete");
//! ```
//!
//! The log will be:
//!
//! ``` text
//! [2025-06-11 14:33:08.089090][DEBUG][request.rs:67] API service started
//! [2025-06-11 14:33:10.099092][DEBUG][request.rs:67] Req / received (000000000000007b)
//! [2025-06-11 14:33:10.099232][WARN][request.rs:68] header xxx (000000000000007b)
//! [2025-06-11 14:33:11.009092][DEBUG][request.rs:67] Req / 200 complete (000000000000007b)
//! ```

extern crate captains_log_helper;
extern crate log;
extern crate signal_hook;

#[macro_use]
extern crate enum_dispatch;

mod config;
mod console_impl;
mod file_impl;
mod formatter;
mod log_impl;
mod time;

pub mod macros;
pub mod parser;
pub mod recipe;

mod log_filter;

pub use self::{config::*, formatter::FormatRecord, log_filter::*, log_impl::setup_log};
pub use captains_log_helper::logfn;

#[cfg(test)]
mod tests;
