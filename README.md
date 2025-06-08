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

* On default supports multi-process/thread/coroutine log into the same file.
Atomic line appending can be done on Linux

* Provides `LogFilter` for coroutine-based programs. You can set req_id in LogFilter and
output to log files

# Dependency

``` toml
[dependencies]
log = { version = "0.4", features = ["std", "kv_unstable"] }
captains_log = "0.1"
```

# Fast setup eample:

```rust
/// #[macro_use]
/// extern crate captains_log;
/// #[macro_use]
/// extern crate log;
use log::{debug, info, error};
use captains_log::*;
use captains_log::recipe::split_error_file_logger;

let log_builder = split_error_file_logger("/tmp", "test", log::Level::Debug);
setup_log(log_builder);

// non-error msg will only appear in /tmp/test.log
debug!("Set a course to Sol system");
info!("Engage");

// will appear in both /tmp/test.log and /tmp/test.log.wf
error!("Engine over heat!");

```

