# captains-log

A minimalist, customizable, easy to use logger for rust, based on the `log` crate, also adapted to `tracing`,
for production and testing scenario.

crates.io: [![crates.io][cratesio-image]][cratesio]
docs.rs: [![docs.rs][docsrs-image]][docsrs]

[cratesio-image]: https://img.shields.io/crates/v/captains-log.svg
[cratesio]: https://crates.io/crates/captains-log
[docsrs-image]: https://docs.rs/captains-log/badge.svg
[docsrs]: https://docs.rs/captains-log

## Features

* Allow customize log format and time format. Refer to [LogFormat](https://docs.rs/captains-log/latest/captains_log/struct.LogFormat.html)

* Dynamic reconfigurable.

* Support [subscribing span and event log](https://docs.rs/captains-log/latest/captains_log/ringfile) from **tracing** (**feature** `tracing`) with consistent format:

    + global default subscriber mode

    + layer mode

    + scoped mode

* Multiple types of sink stacking, each with its own log level.

    + `LogConsole`:  Console output to stdout/stderr.

    + `LogRawFile`:  Support atomic appending from multi-process on linux (with ext4, xfs)

    + [LogBufFile](https://docs.rs/captains-log/latest/captains_log/struct.LogBufFile.html):
  Write to log file with merged I/O and delay flush, and optional self-rotation.

    + [Syslog](https://docs.rs/captains-log/latest/captains_log/struct.Syslog.html): (**feature** `syslog`)

        Write to local or remote syslog server, with timeout and auto reconnect.

    + [LogRingFile](https://docs.rs/captains-log/latest/captains_log/struct.LogRingFile.html): (**feature** `ringfile`)

        For deadlock / race condition debugging, collect log to ring buffer in memory, flush on panic or by signal.

* Log panic message by default.

* Provide additional macros. For example: log_assert!(), logger_assert!() ...

* Supports signal listening for log-rotate. Refer to `Builder::signal()`

* Provides many preset recipes in [recipe]() module for convenience.

* Supports configured by environment

* Fine-grain module-level control and API-level log handling.

  Provides `LogFilter` and `LogFilterKV`  to filter specified logs on-the-fly. Refer to [doc](https://docs.rs/captains-log/latest/captains_log/filter)

* For test suits usage:

  + Allow dynamic reconfigure logger setting in different test function.

    Refer to [Unit test example](https://docs.rs/captains-log/latest/captains_log/#unit-test-example).

  + Provides an attribute macro #\[logfn\] to wrap test function.

    Refer to [Best practice with rstest](https://docs.rs/captains-log/latest/captains_log/#best-practice-with-rstest)

* Provides a `parser` to work on your log files.

## Usage

Cargo.toml

``` toml
[dependencies]
log = { version = "0.4", features = ["std", "kv_unstable"] }
captains_log = "0.13"
```

lib.rs or main.rs:
```

// By default, reexport the macros from log crate
#[macro_use]
extern crate captains_log;
```

## Features flags

- syslog: Enable [Syslog] sink

- ringfile: Enable [LogRingFile] sink

- tracing: Receive log from tracing

...

See detail usage on [docs.rs](https://docs.rs/captains-log)
