# captains-log

A light-weight customizable logger implementation for rust

[![crates.io][cratesio-image]][cratesio]
[![docs.rs][docsrs-image]][docsrs]

[cratesio-image]: https://img.shields.io/crates/v/captains-log.svg
[cratesio]: https://crates.io/crates/captains-log
[docsrs-image]: https://docs.rs/captains-log/badge.svg
[docsrs]: https://docs.rs/captains-log

## Features

* Allow customize log format and time format. Refer to `LogFormat`

* Supports multiple types of sink stacking, each with its own log level.

    + `Builder::console(LogConsole)`:   Console output to stdout/stderr.

    + `Builder::raw_file(LogRawFile)`:  Support atomic appending from multi-process on linux

* Log panic message by default.

* Provide additional macros. For example: log_assert!(), logger_assert!() ...

* Supports signal listening for log-rotate. Refer to `Builder::signal()`

* Provides many preset recipes in [recipe] module for convenience.

* Supports configured from environment.

* Fine-grain module-level log control.

  Provides `LogFilter` to filter specified logs on-the-fly.

* API-level log handling.

  Provides `LogFilterKV` for API logging with additional key.

  For example, you can set `req_id` in `LogFilterKV`, and track the
complete request handling procedure from log.

* For test suits usage:

  Allow dynamic reconfigure logger setting in different test function.

  (NOTE: currently signal_listener does not support reconfigure).

  Provides an attribute macro #[logfn] to wrap test function. Logging test-start and test-end.

* Provides a `parser` to work on your log files.

## Usage

Cargo.toml

``` toml
[dependencies]
log = { version = "0.4", features = ["std", "kv_unstable"] }
captains_log = "0.5"
```

lib.rs or main.rs:
```

// By default, reexport the macros from log crate
#[macro_use]
extern crate captains_log;
```

## Fast setup example

You can refer to various preset recipe in `recipe` module.

The following is setup two log files for different log-level:

``` rust
#[macro_use]
extern crate captains_log;
use captains_log::recipe;

// You'll get /tmp/test.log with all logs, and /tmp/test.log.wf only with error logs.

let mut log_builder = recipe::split_error_file_logger("/tmp", "test", log::Level::Debug);
// Builder::build() is equivalent of setup_log()
log_builder.build();

// non-error msg will only appear in /tmp/test.log
debug!("Set a course to Sol system");
info!("Engage");

// will appear in both /tmp/test.log and /tmp/test.log.wf
error!("Engine over heat!");
```

## Configure by environment

There is a recipe `env_logger()` to configure a file logger or
console logger from env. As simple as:

``` rust
use captains_log::recipe;
let _ = recipe::env_logger("LOG_FILE", "LOG_LEVEL").build();
```

If you want to custom more, setup your config with `env_or()` helper.



## Customize format example

``` rust
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
let debug_file = LogRawFile::new(
    "/tmp", "test.log", log::Level::Trace, debug_format);
let config = Builder::default()
    .signal(signal_hook::consts::SIGINT)
    .raw_file(debug_file);

config.build();
```

## Fine-grain module-level log control

Place `LogFilter` in Arc and share among coroutines.
Log level can be changed on-the-fly.

There're a set of macro "logger_XXX" to work with `LogFilter`.

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

Request log can be track by customizable key (for example, "req_id"), which kept in [LogFilterKV],
and `LogFilterKV` is inherit from `LogFilter`.

You need macro "logger_XXX" to work with it.


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
let builder = recipe::raw_file_logger_custom("/tmp", "log_filter", log::Level::Debug,
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
```

## Unit test example

To setup different log config on different tests.

call <font color=Blue> test() </font> on [Builder],
which enable dynamic log config and disable signal_hook.

```rust
use captains_log::*;

#[test]
fn test1() {
    recipe::raw_file_logger(
        "/tmp", "test1", Level::Debug).test().build();
    info!("doing test1");
}

#[test]
fn test2() {
    recipe::raw_file_logger(
        "/tmp", "test2", Level::Debug).test().build();
    info!("doing test2");
}
```

## Best practice with test suit

We provides proc macro #[logfn], nice to combine with rstest.

* When you have large test suit, you want to know which logs belong to which test case.

* Sometimes your test crashes, you want to find the responsible test case.

``` rust
use rstest::*;
use log::*;
use captains_log::*;

// A show case that setup() fixture will be called twice, before each test.
// In order make logs available.
#[logfn]
#[fixture]
fn setup() {
    let builder = recipe::raw_file_logger("/tmp", "log_rstest", log::Level::Debug).test();
    builder.build().expect("setup_log");
}

#[logfn]
#[rstest(file_size, case(0), case(1))]
fn test_rstest_foo(setup: (), file_size: usize) {
    info!("do something111");
}

#[logfn]
#[rstest]
fn test_rstest_bar(setup: ()) {
    info!("do something222");
}

```

After running the test with:
`cargo test -- --test-threads=1`

/tmp/log_rstest.log will have this content:

``` text
[2025-06-21 00:39:37.091326][INFO][test_rstest.rs:11] >>> setup return () >>>
[2025-06-21 00:39:37.091462][INFO][test_rstest.rs:27] <<< test_rstest_bar (setup = ()) enter <<<
[2025-06-21 00:39:37.091493][INFO][test_rstest.rs:30] do something222
[2025-06-21 00:39:37.091515][INFO][test_rstest.rs:27] >>> test_rstest_bar return () >>>
[2025-06-21 00:39:37.091719][INFO][test_rstest.rs:11] <<< setup () enter <<<
[2025-06-21 00:39:37.091826][INFO][test_rstest.rs:11] >>> setup return () >>>
[2025-06-21 00:39:37.091844][INFO][test_rstest.rs:21] <<< test_rstest_foo (setup = (), file_size = 0) enter <<<
[2025-06-21 00:39:37.091857][INFO][test_rstest.rs:24] do something111
[2025-06-21 00:39:37.091868][INFO][test_rstest.rs:21] >>> test_rstest_foo return () >>>
[2025-06-21 00:39:37.092063][INFO][test_rstest.rs:11] <<< setup () enter <<<
[2025-06-21 00:39:37.092136][INFO][test_rstest.rs:11] >>> setup return () >>>
[2025-06-21 00:39:37.092151][INFO][test_rstest.rs:21] <<< test_rstest_foo (setup = (), file_size = 1) enter <<<
[2025-06-21 00:39:37.092163][INFO][test_rstest.rs:24] do something111
[2025-06-21 00:39:37.092173][INFO][test_rstest.rs:21] >>> test_rstest_foo return () >>>
```
