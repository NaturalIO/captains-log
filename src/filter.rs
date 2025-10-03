//! # Fine-grain log filtering
//!
//! A large application may designed with multiple layers. Sometimes you have many files and modules,
//! and you want more fine-grain controlling for the log, turn on / off by functionality.
//!
//! In order not limit by the number of log level, you can separate `LogFilter` into
//! category, and place [LogFilter] in Arc and share among threads and coroutines.
//! It will become more flexible with the number of `LogFilter` X log_level.
//!
//! When you want to debug the behavior on-the-flay,
//! you can just change log level of a certain `LogFilter` with API.
//!
//! See the doc of [LogFilter] for details.
//!
//! In order For API level tracking, we provide `KeyFilter`, which inherits from `LogFilter`,
//! a custom key can be placed in it. It's like human readable log with structure message.
//! So that you can grep the log with specified request.
//!
//! See the doc of [KeyFilter] for details.

use std::{
    fmt,
    ops::Deref,
    str,
    sync::atomic::{AtomicUsize, Ordering},
};

use log::{kv::*, *};

pub trait Filter: Send + Sized + 'static {
    /// whether a log level is enable
    fn is_enabled(&self, _level: Level) -> bool;

    /// for macros logger_XXX
    #[doc(hidden)]
    #[inline(always)]
    fn _private_api_log(
        &self, args: fmt::Arguments, level: Level,
        &(target, module_path, file, line): &(&str, &str, &str, u32),
    ) {
        let record = RecordBuilder::new()
            .level(level)
            .target(target)
            .module_path(Some(module_path))
            .file(Some(file))
            .line(Some(line))
            .args(args)
            .build();
        logger().log(&record);
    }
}

/// `LogFilter` supports concurrent control the log level filter with atomic.
///
/// Used in combine with macros logger_XXX. the log level filter can be dynamic changed.
///
/// # Example
///
/// ``` rust
/// use std::sync::Arc;
/// use captains_log::{*, filter::LogFilter};
/// log::set_max_level(log::LevelFilter::Debug);
/// let logger_io = Arc::new(LogFilter::new());
/// let logger_req = Arc::new(LogFilter::new());
/// logger_io.set_level(log::Level::Error);
/// logger_req.set_level(log::Level::Debug);
/// logger_debug!(logger_req, "Begin handle req ...");
/// logger_debug!(logger_io, "Issue io to disk ...");
/// logger_error!(logger_req, "Req invalid ...");
/// ```
pub struct LogFilter {
    max_level: AtomicUsize,
}

impl Clone for LogFilter {
    fn clone(&self) -> Self {
        Self { max_level: AtomicUsize::new(self.get_level()) }
    }
}

impl LogFilter {
    pub fn new() -> Self {
        Self { max_level: AtomicUsize::new(Level::Trace as usize) }
    }

    /// When LogFilter is shared in Arc, allows concurrently changing log level filter
    #[inline]
    pub fn set_level(&self, level: Level) {
        self.max_level.store(level as usize, Ordering::Relaxed);
    }

    #[inline]
    pub fn get_level(&self) -> usize {
        self.max_level.load(Ordering::Relaxed)
    }
}

impl Filter for LogFilter {
    #[inline(always)]
    fn is_enabled(&self, level: Level) -> bool {
        level as usize <= self.max_level.load(Ordering::Relaxed)
    }
}

impl log::kv::Source for LogFilter {
    #[inline(always)]
    fn visit<'kvs>(&'kvs self, _visitor: &mut dyn Visitor<'kvs>) -> Result<(), Error> {
        Ok(())
    }

    #[inline(always)]
    fn get<'a>(&'a self, _key: Key) -> Option<Value<'a>> {
        return None;
    }

    #[inline(always)]
    fn count(&self) -> usize {
        0
    }
}

/// A Filter that enables all log levels
pub struct DummyFilter();

impl Filter for DummyFilter {
    #[inline(always)]
    fn is_enabled(&self, _level: Level) -> bool {
        true
    }
}

impl log::kv::Source for DummyFilter {
    #[inline(always)]
    fn visit<'kvs>(&'kvs self, _visitor: &mut dyn Visitor<'kvs>) -> Result<(), Error> {
        Ok(())
    }

    #[inline(always)]
    fn get<'a>(&'a self, _key: Key) -> Option<Value<'a>> {
        return None;
    }

    #[inline(always)]
    fn count(&self) -> usize {
        0
    }
}

/// `KeyFilter` is inherited from [LogFilter], with one additional key into log format.
///
/// The name of the key can be customized.
///
/// Example for an http service, api handling log will have a field `req_id`.
/// When you received error from one of the request,
/// you can grep all the relevant log with that `req_id`.
///
/// ``` rust
/// use captains_log::{*, filter::KeyFilter};
/// fn debug_format_req_id_f(r: FormatRecord) -> String {
///     let time = r.time();
///     let level = r.level();
///     let file = r.file();
///     let line = r.line();
///     let msg = r.msg();
///     let req_id = r.key("req_id");
///     format!("[{time}][{level}][{file}:{line}] {msg}{req_id}\n").to_string()
/// }
/// let builder = recipe::raw_file_logger_custom(
///                 "/tmp/log_filter.log", log::Level::Debug,
///                 recipe::DEFAULT_TIME, debug_format_req_id_f)
///     .build().expect("setup log");
///
/// let logger = KeyFilter::new("req_id", format!("{:016x}", 123).to_string());
/// info!("API service started");
/// logger_debug!(logger, "Req / received");
/// logger_debug!(logger, "header xxx");
/// logger_info!(logger, "Req / 200 complete");
/// ```
///
/// The log will be:
///
/// ``` text
/// [2025-06-11 14:33:08.089090][DEBUG][request.rs:67] API service started
/// [2025-06-11 14:33:10.099092][DEBUG][request.rs:67] Req / received (000000000000007b)
/// [2025-06-11 14:33:10.099232][WARN][request.rs:68] header xxx (000000000000007b)
/// [2025-06-11 14:33:11.009092][DEBUG][request.rs:67] Req / 200 complete (000000000000007b)
/// ```
#[derive(Clone)]
pub struct KeyFilter {
    inner: LogFilter,
    key: &'static str,
    value: String,
}

impl KeyFilter {
    pub fn new(key: &'static str, value: String) -> Self {
        Self { inner: LogFilter::new(), key, value }
    }
}

impl log::kv::Source for KeyFilter {
    #[inline(always)]
    fn visit<'kvs>(&'kvs self, visitor: &mut dyn Visitor<'kvs>) -> Result<(), Error> {
        visitor.visit_pair(self.key.to_key(), self.value.as_str().into())
    }

    #[inline(always)]
    fn get<'a>(&'a self, key: Key) -> Option<Value<'a>> {
        if key.as_ref() == self.key {
            return Some(self.value.as_str().into());
        }
        return None;
    }

    #[inline(always)]
    fn count(&self) -> usize {
        1
    }
}

impl Deref for KeyFilter {
    type Target = LogFilter;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Filter for KeyFilter {
    #[inline(always)]
    fn is_enabled(&self, level: Level) -> bool {
        self.inner.is_enabled(level)
    }

    /// for macros logger_XXX
    #[doc(hidden)]
    #[inline(always)]
    fn _private_api_log(
        &self, args: fmt::Arguments, level: Level,
        &(target, module_path, file, line): &(&str, &str, &str, u32),
    ) {
        let record = RecordBuilder::new()
            .level(level)
            .target(target)
            .module_path(Some(module_path))
            .file(Some(file))
            .line(Some(line))
            .key_values(&self)
            .args(args)
            .build();
        logger().log(&record);
    }
}
