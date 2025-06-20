use std::{
    fmt, str,
    sync::atomic::{AtomicUsize, Ordering},
};

use log::{kv::*, *};

/// A LogFilter supports concurrent control the log level.
/// Use in combine with macros logger_XXX
///
/// # Example
/// ```
/// use captains_log::*;
/// let logger = LogFilter::new();
/// logger.set_level(log::Level::Error);
/// // info will be filtered
/// logger_info!(logger, "using LogFilter {}", "ok");
/// logger_error!(logger, "error occur");
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

    /// for macros logger_XXX
    #[doc(hidden)]
    #[inline(always)]
    pub fn _private_api_log(
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

/// LogFilter that carries one additional value into log format
///
/// Example with a key as "req_id":
/// ``` rust
/// use captains_log::*;
/// fn debug_format_req_id_f(r: FormatRecord) -> String {
///     let time = r.time();
///     let level = r.level();
///     let file = r.file();
///     let line = r.line();
///     let msg = r.msg();
///     let req_id = r.key("req_id");
///     format!("[{time}][{level}][{file}:{line}] {msg}{req_id}\n").to_string()
/// }
/// let mut builder = recipe::raw_file_logger_custom("/tmp", "log_filter", log::Level::Debug,
///     recipe::DEFAULT_TIME, debug_format_req_id_f);
/// builder.dynamic = true;
///
/// builder.build().expect("setup_log");
/// let logger = LogFilterKV::new("req_id", format!("{:016x}", 123).to_string());
/// logger_debug!(logger, "captain's log");
/// ```

#[derive(Clone)]
pub struct LogFilterKV {
    inner: LogFilter,
    key: &'static str,
    value: String,
}

impl LogFilterKV {
    pub fn new(key: &'static str, value: String) -> Self {
        Self { inner: LogFilter::new(), key, value }
    }

    /// When LogFilter is shared in Arc, allows concurrently changing log level filter
    #[inline]
    pub fn set_level(&self, level: Level) {
        self.inner.set_level(level)
    }

    #[inline]
    pub fn get_level(&self) -> usize {
        self.inner.get_level()
    }

    /// for macros logger_XXX
    #[doc(hidden)]
    #[inline(always)]
    pub fn _private_api_log(
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

impl log::kv::Source for LogFilterKV {
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
