#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(docsrs, allow(unused_attributes))]

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
//!     + [LogConsole]:  Console output to stdout/stderr.
//!
//!     + [LogRawFile]:  Support atomic appending from multi-process on linux
//!
//!     + [LogBufFile]:  Write to log file with merged I/O and delay flush, and optional self-rotation.
//!
//!     + [Syslog]: (feature flag `syslog`) Write to local or remote syslog server, with timeout and auto reconnect.
//!
//! * Log panic message by default.
//!
//! * Provide additional [macros](#macros), for example: log_assert!(), logger_assert!() ..
//!
//! * Supports signal listening for log-rotate. Refer to [Builder::signal()]
//!
//! * Provides many preset recipes in [recipe] module for convenience.
//!
//! * [Supports configure by environment](#configure-by-environment)
//!
//! * [Fine-grain module-level control](#fine-grain-module-level-control)
//!
//! * [API-level log handling](#api-level-log-handling)
//!
//! * For test suits usage:
//!
//!     + Allow dynamic reconfigure logger setting in different test function.
//!
//!       Refer to [Unit test example](#unit-test-example).
//!
//!     + Provides an attribute macro #\[logfn\] to wrap test function.
//!
//!       Refer to [Best practice with rstest](#best-practice-with-rstest).
//!
//! * Provides a [LogParser](crate::parser::LogParser) to work on your log files.
//!
//! ## Usage
//!
//! Cargo.toml
//!
//! ``` toml
//! [dependencies]
//! log = { version = "0.4", features = ["std", "kv_unstable"] }
//! captains_log = "0.7"
//! ```
//!
//! lib.rs or main.rs:
//!
//! ``` rust
//!
//! // By default, reexport the macros from log crate
//! #[macro_use]
//! extern crate captains_log;
//!
//! ```
//!
//! ## Fast setup examples
//!
//! You can refer to various preset recipe in [recipe] module.
//!
//! The following is setup two log files for different log-level:
//!
//! ``` rust
//! #[macro_use]
//! extern crate captains_log;
//! use captains_log::recipe;
//!
//! // You'll get /tmp/test.log with all logs, and /tmp/test.log.wf only with error logs.
//! let log_builder = recipe::split_error_file_logger("/tmp", "test", log::Level::Debug);
//! // Builder::build() is equivalent of setup_log().
//! log_builder.build();
//! // non-error msg will only appear in /tmp/test.log
//! debug!("Set a course to Sol system");
//! info!("Engage");
//! // will appear in both /tmp/test.log and /tmp/test.log.wf
//! error!("Engine over heat!");
//! ```
//!
//! Buffered sink with log rotation (See the definition of [Rotation]):
//!
//! ``` rust
//! #[macro_use]
//! extern crate captains_log;
//! use captains_log::*;
//! // rotate when log file reaches 512M. Keep max 10 archiveed files, with recent 2 not compressed.
//! // All archived log is moved to "/tmp/rotation/old"
//! let rotation = Rotation::by_size(512 * 1024 * 1024, Some(10))
//!     .compress_exclude(2).archive_dir("/tmp/rotation/old");
//! let _ = recipe::buffered_rotated_file_logger("/tmp/rotation.log", Level::Debug, rotation).build();
//! ```
//!
//! ## Configure by environment
//!
//! There is a recipe [env_logger()](crate::recipe::env_logger()) to configure a file logger or
//! console logger from env. As simple as:
//!
//! ``` rust
//! use captains_log::recipe;
//! let _ = recipe::env_logger("LOG_FILE", "LOG_LEVEL").build();
//! ```
//!
//! If you want to custom more, setup your config with [env_or] helper.
//!
//! ## Customize format example
//!
//! ``` rust
//! use captains_log::*;
//!
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
//! config.build();
//! ```
//!
//! ## Fine-grain module-level control
//!
//! Place [LogFilter] in Arc and share among coroutines.
//! Log level can be changed on-the-fly.
//!
//! There're a set of macro "logger_XXX" to work with `LogFilter`.
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
//! ```
//!
//!
//! ## API-level log handling
//!
//! Request log can be track by customizable key (for example, "req_id"), which kept in [LogFilterKV],
//! and `LogFilterKV` is inherit from `LogFilter`.
//! You need macro "logger_XXX" to work with it.
//!
//! ``` rust
//! use captains_log::*;
//! fn debug_format_req_id_f(r: FormatRecord) -> String {
//!     let time = r.time();
//!     let level = r.level();
//!     let file = r.file();
//!     let line = r.line();
//!     let msg = r.msg();
//!     let req_id = r.key("req_id");
//!     format!("[{time}][{level}][{file}:{line}] {msg}{req_id}\n").to_string()
//! }
//! let builder = recipe::raw_file_logger_custom("/tmp/log_filter.log", log::Level::Debug,
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
//!
//!
//! ## Unit test example
//!
//! To setup different log config on different tests.
//!
//! **Make sure that you call [Builder::test()]** in test cases.
//! which enable dynamic log config and disable signal_hook.
//!
//! ```rust
//! use captains_log::*;
//!
//! #[test]
//! fn test1() {
//!     recipe::raw_file_logger(
//!         "/tmp/test1.log", Level::Debug).test().build();
//!     info!("doing test1");
//! }
//!
//! #[test]
//! fn test2() {
//!     recipe::raw_file_logger(
//!         "/tmp/test2.log", Level::Debug).test().build();
//!     info!("doing test2");
//! }
//! ```
//!
//! ## Best practice with rstest
//!
//! We provides proc macro [logfn], the following example shows how to combine with rstest.
//!
//! * When you have large test suit, you want to know which logs belong to which test case.
//!
//! * Sometimes your test crashes, you want to find the responsible test case.
//!
//! * The time spend in each test.
//!
//! ``` rust
//!
//! use rstest::*;
//! use captains_log::*;
//!
//! // A show case that setup() fixture will be called twice, before each test.
//! // In order make logs available.
//! #[fixture]
//! fn setup() {
//!     let builder = recipe::raw_file_logger("/tmp/log_rstest.log", log::Level::Debug).test();
//!     builder.build().expect("setup_log");
//! }
//!
//! #[logfn]
//! #[rstest(file_size, case(0), case(1))]
//! fn test_rstest_foo(setup: (), file_size: usize) {
//!     info!("do something111");
//! }
//!
//! #[logfn]
//! #[rstest]
//! fn test_rstest_bar(setup: ()) {
//!     info!("do something222");
//! }
//!
//! // NOTE rstest must be at the bottom to make fixture effective
//! #[tokio::test]
//! #[logfn]
//! #[rstest]
//! async fn test_rstest_async(setup: ()) {
//!     info!("something333")
//! }
//! ```
//!
//! **Notice:** the order when combine tokio::test with rstest,
//! `#[rstest]` attribute must be at the bottom to make setup fixture effective.
//!
//! After running the test with:
//!
//! cargo test -- --test-threads=1
//!
//! /tmp/log_rstest.log will have this content:
//!
//! ``` text
//! [2025-07-13 18:22:39.159642][INFO][test_rstest.rs:33] <<< test_rstest_async (setup = ()) enter <<<
//! [2025-07-13 18:22:39.160255][INFO][test_rstest.rs:37] something333
//! [2025-07-13 18:22:39.160567][INFO][test_rstest.rs:33] >>> test_rstest_async return () in 564.047µs >>>
//! [2025-07-13 18:22:39.161299][INFO][test_rstest.rs:26] <<< test_rstest_bar (setup = ()) enter <<<
//! [2025-07-13 18:22:39.161643][INFO][test_rstest.rs:29] do something222
//! [2025-07-13 18:22:39.161703][INFO][test_rstest.rs:26] >>> test_rstest_bar return () in 62.681µs >>>
//! [2025-07-13 18:22:39.162169][INFO][test_rstest.rs:20] <<< test_rstest_foo (setup = (), file_size = 0) enter <<<
//! [2025-07-13 18:22:39.162525][INFO][test_rstest.rs:23] do something111
//! [2025-07-13 18:22:39.162600][INFO][test_rstest.rs:20] >>> test_rstest_foo return () in 78.457µs >>>
//! [2025-07-13 18:22:39.163050][INFO][test_rstest.rs:20] <<< test_rstest_foo (setup = (), file_size = 1) enter <<<
//! [2025-07-13 18:22:39.163320][INFO][test_rstest.rs:23] do something111
//! [2025-07-13 18:22:39.163377][INFO][test_rstest.rs:20] >>> test_rstest_foo return () in 58.747µs >>>
//! ```

extern crate captains_log_helper;
extern crate log;
extern crate signal_hook;

#[macro_use]
extern crate enum_dispatch;

mod buf_file_impl;
mod config;
mod console_impl;
mod env;
mod file_impl;
mod formatter;
mod log_impl;
mod rotation;
#[cfg(feature = "syslog")]
#[cfg_attr(docsrs, doc(cfg(feature = "syslog")))]
mod syslog;
mod time;
#[cfg(feature = "syslog")]
#[cfg_attr(docsrs, doc(cfg(feature = "syslog")))]
pub use self::syslog::*;

pub mod macros;
pub mod parser;
pub mod recipe;

mod log_filter;

pub use self::buf_file_impl::*;
pub use self::console_impl::*;
pub use self::env::*;
pub use self::file_impl::*;
pub use self::rotation::*;
pub use self::{config::*, formatter::FormatRecord, log_filter::*, log_impl::setup_log};
pub use captains_log_helper::logfn;

pub use log::{debug, error, info, trace, warn};
pub use log::{Level, LevelFilter};

#[cfg(test)]
mod tests;
