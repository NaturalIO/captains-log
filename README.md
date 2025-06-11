# captains-log

A light-weight customizable logger implementation for rust

[![crates.io][cratesio-image]][cratesio]
[![docs.rs][docsrs-image]][docsrs]

[cratesio-image]: https://img.shields.io/crates/v/captains-log.svg
[cratesio]: https://crates.io/crates/captains-log
[docsrs-image]: https://docs.rs/captains-log/badge.svg
[docsrs]: https://docs.rs/captains-log

## Features

* Allow customize log format and time format.

* Supports signal listening for log-rotate.

* Supports multiple log files, each with its own log level.

* Supports hook on panic.

* Supports multi-process/thread/coroutines

  Atomic line appending into the same file can be done on Linux

* Fine-grain module-level log control.

  Provides `LogFilter` to filter specified logs on-the-fly

* API-level log handling.

  Provides `LogFilterKV` for API logging with additional key.

  For example, you can set `req_id` in `LogFilterKV`, and track the
complete request handling procedure from log.

* For test suits usage:

  Allow dynamic reconfigure logger setting in different test function.

  (NOTE: currently signal_listener does not support reconfigure).

  Provides an attribute macro #[logfn] to wrap test function. Logging test-start and test-end.

* Provides a `LogParser` to work on your log files.

## Dependency

``` toml
[dependencies]
log = { version = "0.4", features = ["std", "kv_unstable"] }
captains_log = "0.2"
```

## Fast setup example:

You can refer to various preset recipe in `recipe` module, including console & file output.

```rust
// #[macro_use]
// extern crate captains_log;
// #[macro_use]
// extern crate log;

use log::{debug, info, error};
use captains_log::recipe::split_error_file_logger;

let log_builder = split_error_file_logger("/tmp", "test", log::Level::Debug);
log_builder.build();

// non-error msg will only appear in /tmp/test.log
debug!("Set a course to Sol system");
info!("Engage");

// will appear in both /tmp/test.log and /tmp/test.log.wf
error!("Engine over heat!");

```
## Customize format example

``` rust
extern crate signal_hook;
extern crate chrono;
use captains_log::*;

fn format_f(r: FormatRecord) -> String {
    let time = r.time();
    let level = r.level();
    let file = r.file();
    let line = r.line();
    let msg = r.msg();
    format!("{time}|{level}|{file}:{line}|{msg}\n").to_string()
}
let debug_format = LogFormat::new(
    "%Y%m%d %H:%M:%S%.6f",
    format_f,
);
let debug_file = LogFile::new(
    "/tmp", "test.log", log::Level::Trace, debug_format);
let config = Builder::default()
    .signal(signal_hook::consts::SIGINT)
    .file(debug_file);

config.build();
```

## Fine-grain module-level log control

Place `LogFilter` in Arc and share among coroutines.
Log level can be changed on-the-fly.

``` rust
use std::sync::Arc;
use captains_log::*;
log::set_max_level(log::LevelFilter::Debug);
let logger_io = Arc::new(LogFilter::new());
let logger_req = Arc::new(LogFilter::new());
logger_io.set_level(log::Level::Error);
logger_req.set_level(log::Level::Debug);
logger_debug!(logger_req, "Begin handle req ...");
logger_debug!(logger_io, "Issue io to disk ...");
logger_error!(logger_req, "Req invalid ...");

```

## API-level log handling

Request log can be track by custom key `req_id`, which kept in `LogFilterKV`.

``` rust
use captains_log::*;
fn debug_format_req_id_f(r: FormatRecord) -> String {
    let time = r.time();
    let level = r.level();
    let file = r.file();
    let line = r.line();
    let msg = r.msg();
    let req_id = r.key("req_id");
    format!("[{time}][{level}][{file}:{line}] {msg}{req_id}\n").to_string()
}
let builder = recipe::file_logger_custom("/tmp", "log_filter.log", log::Level::Debug,
    recipe::DEFAULT_TIME, debug_format_req_id_f);
builder.build().expect("setup_log");
let logger = LogFilterKV::new("req_id", format!("{:016x}", 123).to_string());
logger_debug!(logger, "Req / received");
logger_debug!(logger, "header xxx");
logger_info!(logger, "Req / 200 complete");

```
The log will be:

```
[2025-06-11 14:33:08.089090][DEBUG][request.rs:67] API service started
[2025-06-11 14:33:10.099092][DEBUG][request.rs:67] Req / received (000000000000007b)
[2025-06-11 14:33:10.099232][WARN][request.rs:68] header xxx (000000000000007b)
[2025-06-11 14:33:11.009092][DEBUG][request.rs:67] Req / 200 complete (000000000000007b)

