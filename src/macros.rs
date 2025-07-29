#[doc(hidden)]
#[macro_export]
macro_rules! do_log_filter {
    (target: $target:expr, $log_filter:expr, $lvl:expr, $($arg:tt)+) => ({
        let lvl = $lvl;
        if lvl <= log::STATIC_MAX_LEVEL && lvl as usize <= $log_filter.get_level() && lvl <= log::max_level() {
            $log_filter._private_api_log(
                std::format_args!($($arg)+),
                lvl,
                &($target, std::module_path!(), std::file!(), std::line!()),
            );
        }
    });
    ($log_filter:expr, $lvl:expr, $($arg:tt)+) => (do_log_filter!(target: std::module_path!(), $log_filter, $lvl, $($arg)+))
}
#[allow(unused_imports)]
pub(super) use do_log_filter;

/// Similar to [error!()](log::error!()), but the first argument is [LogFilter](crate::LogFilter) or
/// [LogFilterKV](crate::LogFilterKV).
#[macro_export]
macro_rules! logger_error {
    ($log_filter:expr, $($arg:tt)+) => (
        do_log_filter!($log_filter, log::Level::Error, $($arg)+);
    )
}
#[allow(unused_imports)]
pub(super) use logger_error;

/// Similar to [warn!()](log::warn!()), but the first argument is [LogFilter](crate::LogFilter) or
/// [LogFilterKV](crate::LogFilterKV).
#[macro_export]
macro_rules! logger_warn {
    ($log_filter:expr, $($arg:tt)+) => (
        do_log_filter!($log_filter, log::Level::Warn, $($arg)+);
    )
}
#[allow(unused_imports)]
pub(super) use logger_warn;

/// Similar to [info!()](log::info!()), but the first argument is [LogFilter](crate::LogFilter) or
/// [LogFilterKV](crate::LogFilterKV).
#[macro_export]
macro_rules! logger_info {
    ($log_filter:expr, $($arg:tt)+) => (
        do_log_filter!($log_filter, log::Level::Info, $($arg)+);
    )
}
#[allow(unused_imports)]
pub(super) use logger_info;

/// Similar to [debug!()](log::debug!()), but the first argument is [LogFilter](crate::LogFilter) or
/// [LogFilterKV](crate::LogFilterKV)
#[macro_export]
macro_rules! logger_debug {
    ($log_filter:expr, $($arg:tt)+) => (
        do_log_filter!($log_filter, log::Level::Debug, $($arg)+);
    )
}
#[allow(unused_imports)]
pub(super) use logger_debug;

/// Similar to [trace!()](log::trace!()), but the first argument is [LogFilter](crate::LogFilter) or
/// [LogFilterKV](crate::LogFilterKV)
#[macro_export]
macro_rules! logger_trace {
    ($log_filter:expr, $($arg:tt)+) => (
        do_log_filter!($log_filter, log::Level::Trace, $($arg)+);
    )
}
#[allow(unused_imports)]
pub(super) use logger_trace;

/// On debug build, will log with log_filter and panic when condition not met. Skip the check on
/// release build.
///
/// The first argument is [LogFilter](crate::LogFilter) or [LogFilterKV](crate::LogFilterKV), the rest arguments are like [core::debug_assert!()].
///
/// # Examples:
///
/// ``` rust
/// use captains_log::*;
/// let logger = LogFilterKV::new("req_id", format!("{:016x}", 123).to_string());
/// let started = true;
/// logger_debug_assert!(logger, started);
/// logger_debug_assert!(logger, started, "job must have been started");
/// ```
#[macro_export]
macro_rules! logger_debug_assert {
    ($log_filter:expr, $($arg:tt)*) => (if std::cfg!(debug_assertions) { $crate::logger_assert!($log_filter, $($arg)*); });
}
#[allow(unused_imports)]
pub(super) use logger_debug_assert;

/// On debug build, will log with log_filter and panic when condition not met. Skip the check on
/// release build.
///
/// The first argument is [LogFilter](crate::LogFilter) or [LogFilterKV](crate::LogFilterKV), the rest arguments are like [core::debug_assert_eq!()].
///
/// # Examples:
///
/// ``` rust
/// use captains_log::*;
/// let logger = LogFilterKV::new("req_id", format!("{:016x}", 123).to_string());
/// logger_debug_assert_eq!(logger, 1, 1);
/// logger_debug_assert_eq!(logger, 1, 1, "impossible things happended: {}", "haha");
/// ```
#[macro_export]
macro_rules! logger_debug_assert_eq {
    ($log_filter:expr, $($arg:tt)*) => (if std::cfg!(debug_assertions) { $crate::logger_assert_eq!($log_filter, $($arg)*); })
}
#[allow(unused_imports)]
pub(super) use logger_debug_assert_eq;

/// Will log with log_filter and panic when condition not met.
///
/// The first argument is [LogFilter](crate::LogFilter) or [LogFilterKV](crate::LogFilterKV), the rest arguments are like [core::assert!()].
///
/// # Examples:
///
/// ``` rust
/// use captains_log::*;
/// let logger = LogFilterKV::new("req_id", format!("{:016x}", 123).to_string());
/// let user_id = Some(111);
/// logger_assert!(logger, user_id.is_some());
/// logger_assert!(logger, user_id.is_some(), "user must login");
/// ```
#[macro_export]
macro_rules! logger_assert {
    ($log_filter:expr, $cond:expr) => ({
        if !$cond {
            do_log_filter!(
                $log_filter,
                log::Level::Error,
                "assertion failed: {:?}",
                $cond
            );
            std::panic!(r#"assertion failed: {:?}"#, $cond);
        }
    });
    ($log_filter:expr, $cond:expr,) => ({
        $crate::logger_assert!($log_filter, $cond);
    });
    ($log_filter:expr, $cond:expr, $($arg:tt)+) => ({
        if !$cond {
            do_log_filter!(
                $log_filter,
                log::Level::Error,
                "assertion failed: {}",
                std::format_args!($($arg)+)
            );
            std::panic!(r#"{}"#, std::format_args!($($arg)+));
        }
    });
}
#[allow(unused_imports)]
pub(super) use logger_assert;

/// Will log with log_filter and panic when condition not met.
///
/// The first argument is [LogFilter](crate::LogFilter) or [LogFilterKV](crate::LogFilterKV), the rest arguments are like [core::assert_eq!()].
///
/// # Examples:
///
/// ``` rust
/// use captains_log::*;
/// let logger = LogFilterKV::new("req_id", format!("{:016x}", 123).to_string());
/// logger_assert_eq!(logger, 1, 1);
/// logger_assert_eq!(logger, 1, 1, "impossible things happended");
/// ```
#[macro_export]
macro_rules! logger_assert_eq {
    ($log_filter:expr, $left:expr, $right:expr) => ({
        match (&$left, &$right) {
            (left_val, right_val) => {
                if !(*left_val == *right_val) {
                    do_log_filter!($log_filter, log::Level::Error, "assertion failed! \
                    expected: (`left == right`) actual: (`{:?}` != `{:?}`)", &*left_val, &*right_val);
                    std::panic!(r#"assertion failed: `(left == right)`
  left: `{:?}`,
 right: `{:?}`"#, &*left_val, &*right_val);
                }
            }
        }
    });
    ($log_filter:expr, $left:expr, $right:expr,) => ({
        $crate::logger_assert_eq!($log_filter, $left, $right);
    });
    ($log_filter:expr, $left:expr, $right:expr, $($arg:tt)+) => ({
        match (&($left), &($right)) {
            (left_val, right_val) => {
                if !(*left_val == *right_val) {
                    do_log_filter!($log_filter, log::Level::Error, "assertion failed! \
                    expected: `(left == right)` actual: (`{:?}` != `{:?}`)", &*left_val, &*right_val);
                    std::panic!(r#"assertion failed: `(left == right)`
  left: `{:?}`,
 right: `{:?}`: {}"#, &*left_val, &*right_val,
                           std::format_args!($($arg)+));
                }
            }
        }
    });
}
#[allow(unused_imports)]
pub(super) use logger_assert_eq;

/// On Debug build, will log and panic when condition not met. Skip the check on
/// release build.
///
/// The arguments are like [core::debug_assert!()].
///
/// # Examples:
///
/// ``` rust
/// use captains_log::*;
/// let user_id = Some(111);
/// log_debug_assert!(user_id.is_some());
/// log_debug_assert!(user_id.is_some(), "user must login");
/// ```
#[macro_export]
macro_rules! log_debug_assert {
    ($($arg:tt)*) => (if std::cfg!(debug_assertions) { $crate::log_assert!($($arg)*); });
}
#[allow(unused_imports)]
pub(super) use log_debug_assert;

/// On Debug build, will log and panic when condition not met. Skip the check on
/// release build.
///
/// The arguments are like [core::debug_assert_eq!()].
///
/// # Examples:
///
/// ``` rust
/// use captains_log::*;
/// log_debug_assert_eq!(1, 1);
/// log_debug_assert_eq!(1, 1, "impossible things happended");
/// ```
#[macro_export]
macro_rules! log_debug_assert_eq {
    ($($arg:tt)*) => (if std::cfg!(debug_assertions) { $crate::log_assert_eq!($($arg)*); })
}
#[allow(unused_imports)]
pub(super) use log_debug_assert_eq;

/// Will log and panic when condition not met.
///
/// The arguments are like [core::assert!()].
///
/// # Examples:
///
/// ``` rust
/// use captains_log::*;
/// let user_id = Some(111);
/// log_assert!(user_id.is_some());
/// log_assert!(user_id.is_some(), "user must login");
/// ```
#[macro_export]
macro_rules! log_assert {
    ($cond:expr) => ({
        if !$cond {
            log::error!(
                "assertion failed: {:?}",
                $cond
            );
            std::panic!(r#"assertion failed: {:?}"#, $cond);
        }
    });
    ($cond:expr,) => ({
        $crate::log_assert!($log_filter, $cond);
    });
    ($cond:expr, $($arg:tt)+) => ({
        if !$cond {
            log::error!(
                "assertion failed: {}",
                std::format_args!($($arg)+)
            );
            std::panic!(r#"{}"#, std::format_args!($($arg)+));
        }
    });
}
#[allow(unused_imports)]
pub(super) use log_assert;

/// Will log and panic when condition not met.
///
/// The arguments are like [core::assert_eq!()].
///
/// # Examples:
///
/// ``` rust
/// use captains_log::*;
/// log_assert_eq!(1, 1);
/// log_assert_eq!(1, 1, "impossible things happended");
/// ```
#[macro_export]
macro_rules! log_assert_eq {
    ($left:expr, $right:expr) => ({
        match (&$left, &$right) {
            (left_val, right_val) => {
                if !(*left_val == *right_val) {
                    log::error!("assertion failed! \
                    expected: (`left == right`) actual: (`{:?}` != `{:?}`)", &*left_val, &*right_val);
                    std::panic!(r#"assertion failed: `(left == right)`
  left: `{:?}`,
 right: `{:?}`"#, &*left_val, &*right_val);
                }
            }
        }
    });
    ($left:expr, $right:expr,) => ({
        $crate::log_assert_eq!($left, $right);
    });
    ($left:expr, $right:expr, $($arg:tt)+) => ({
        match (&($left), &($right)) {
            (left_val, right_val) => {
                if !(*left_val == *right_val) {
                    log::error!( "assertion failed! \
                    expected: `(left == right)` actual: (`{:?}` != `{:?}`)", &*left_val, &*right_val);
                    std::panic!(r#"assertion failed: `(left == right)`
  left: `{:?}`,
 right: `{:?}`: {}"#, &*left_val, &*right_val,
                           std::format_args!($($arg)+));
                }
            }
        }
    });
}

#[allow(unused_imports)]
pub(super) use log_assert_eq;

/// log and println to stdout.
///
/// The usage is simular to [std::println!()]
#[macro_export]
macro_rules! log_println {
    ($($arg:tt)+) => {
        std::println!($($arg)+);
        log::info!($($arg)+);
    }
}
#[allow(unused_imports)]
pub(super) use log_println;

/// log and println to stderr.
///
/// The usage is simular to [std::eprintln!()]
#[macro_export]
macro_rules! log_eprintln {
    ($($arg:tt)+) => {
        std::eprintln!($($arg)+);
        log::info!($($arg)+);
    }
}
#[allow(unused_imports)]
pub(super) use log_eprintln;
