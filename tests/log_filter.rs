use captains_log::{recipe::split_error_file_logger, *};
use regex::Regex;
use std::fs::*;
use std::panic;

mod common;
use common::*;

const RE_DEBUG: &str = r"^\[(.+)\]\[(\w+)\]\[(.+)\:(\d+)\] (.+)$";

#[test]
fn test_logger_filter() {
    lock_file!();

    let mut builder = split_error_file_logger("/tmp", "log_filter", Level::Trace);
    builder.dynamic = true;
    clear_test_files(&builder);
    setup_log(builder).expect("setup_log");

    let logger = LogFilter::new();
    logger_trace!(logger, "hahaha {} {}", "hello", "world");
    logger_debug!(logger, "captain's log");
    logger.set_level(Level::Info);
    logger_debug!(logger, "expect to be filter!");
    logger_info!(logger, "Make it so");
    logger_warn!(logger, "Fire phasers!");

    let debug_logs = parse_log("/tmp/log_filter.log", RE_DEBUG).expect("parse log");
    assert_eq!(debug_logs.len(), 4);
    assert_eq!(debug_logs[0][2], "TRACE");
    assert_eq!(debug_logs[1][2], "DEBUG");
    assert_eq!(debug_logs[2][2], "INFO");
    assert_eq!(debug_logs[3][2], "WARN");
}

#[test]
fn test_logger_filter_kv() {
    lock_file!();

    const RE_DEBUG_REQ: &str = r"^\[(.+)\]\[(\w+)\]\[(.+)\:(\d+)\] (.+?)( \((\w+)\))?$";

    fn debug_format_req_id_f(r: FormatRecord) -> String {
        let time = r.time();
        let level = r.level();
        let file = r.file();
        let line = r.line();
        let msg = r.msg();
        let req_id = r.key("req_id");
        format!("[{time}][{level}][{file}:{line}] {msg}{req_id}\n").to_string()
    }
    let mut builder = recipe::raw_file_logger_custom(
        "/tmp/log_filter.log",
        Level::Debug,
        recipe::DEFAULT_TIME,
        debug_format_req_id_f,
    );
    builder.dynamic = true;
    clear_test_files(&builder);

    builder.build().expect("setup_log");
    let logger = LogFilterKV::new("req_id", format!("{:016x}", 123).to_string());
    // trace not in global max_level
    logger_trace!(logger, "trace should be filtered");
    logger_debug!(logger, "captain's log");
    warn!("fleet broadcast");

    let debug_logs = parse_log("/tmp/log_filter.log", RE_DEBUG_REQ).expect("parse log");
    assert_eq!(debug_logs.len(), 2);
    assert_eq!(debug_logs[0][2], "DEBUG");
    assert_eq!(debug_logs[0][5], "captain's log");
    assert_eq!(debug_logs[0][7], "000000000000007b");
    assert_eq!(debug_logs[1][5], "fleet broadcast");
    assert_eq!(debug_logs[1][7], ""); // global log has no req_id
}

#[test]
fn test_logger_assert_without_msg() {
    lock_file!();

    let mut builder = split_error_file_logger("/tmp", "log_filter", Level::Trace);
    builder.dynamic = true;
    clear_test_files(&builder);
    setup_log(builder).expect("setup_log");
    let logger = LogFilter::new();
    logger_assert!(logger, true);
    let debug_logs = parse_log("/tmp/log_filter.log", RE_DEBUG).expect("parse log");
    assert_eq!(debug_logs.len(), 0);
    let r = panic::catch_unwind(|| {
        log_assert!(false);
    });
    assert!(r.is_err());
    let debug_logs = parse_log("/tmp/log_filter.log", RE_DEBUG).expect("parse log");
    assert_eq!(debug_logs[0][5], "assertion failed: false");
}

#[test]
fn test_logger_assert_with_msg() {
    lock_file!();

    let mut builder = split_error_file_logger("/tmp", "log_filter", Level::Trace);
    builder.dynamic = true;
    clear_test_files(&builder);
    setup_log(builder).expect("setup_log");

    let logger = LogFilter::new();
    logger_assert!(logger, true, "my assert {}", "failed!");
    let debug_logs = parse_log("/tmp/log_filter.log", RE_DEBUG).expect("parse log");
    assert_eq!(debug_logs.len(), 0);
    let r = panic::catch_unwind(|| {
        logger_assert!(logger, false, "my assert {}", "failed!");
    });
    assert!(r.is_err());
    let debug_logs = parse_log("/tmp/log_filter.log", RE_DEBUG).expect("parse log");
    assert_eq!(debug_logs[0][5], "assertion failed: my assert failed!");
}

#[test]
fn test_logger_debug_assert_without_msg() {
    lock_file!();

    let mut builder = split_error_file_logger("/tmp", "log_filter", Level::Trace);
    builder.dynamic = true;
    clear_test_files(&builder);
    setup_log(builder).expect("setup_log");
    let logger = LogFilter::new();
    // Change the following condition to see the result
    logger_debug_assert!(logger, true);
    let debug_logs = parse_log("/tmp/log_filter.log", RE_DEBUG).expect("parse log");
    assert_eq!(debug_logs.len(), 0);
    let r = panic::catch_unwind(|| {
        logger_debug_assert!(logger, false);
    });
    #[cfg(debug_assertions)]
    {
        assert!(r.is_err());
        let debug_logs = parse_log("/tmp/log_filter.log", RE_DEBUG).expect("parse log");
        assert_eq!(debug_logs[0][5], "assertion failed: false");
    }
    #[cfg(not(debug_assertions))]
    {
        // On release the debug_assert is completely bypassed
        assert!(r.is_ok());
        let debug_logs = parse_log("/tmp/log_filter.log", RE_DEBUG).expect("parse log");
        assert_eq!(debug_logs.len(), 0);
    }
}

#[test]
fn test_logger_debug_assert_with_msg() {
    lock_file!();

    let mut builder = split_error_file_logger("/tmp", "log_filter", Level::Trace);
    builder.dynamic = true;
    clear_test_files(&builder);
    setup_log(builder).expect("setup_log");
    let logger = LogFilter::new();
    logger_debug_assert!(logger, true, "my assertion");
    let debug_logs = parse_log("/tmp/log_filter.log", RE_DEBUG).expect("parse log");
    assert_eq!(debug_logs.len(), 0);
    let r = panic::catch_unwind(|| {
        logger_debug_assert!(logger, false, "my assertion");
    });
    #[cfg(debug_assertions)]
    {
        assert!(r.is_err());
        let debug_logs = parse_log("/tmp/log_filter.log", RE_DEBUG).expect("parse log");
        assert_eq!(debug_logs[0][5], "assertion failed: my assertion");
    }
    #[cfg(not(debug_assertions))]
    {
        // On release the debug_assert is completely bypassed
        assert!(r.is_ok());
        let debug_logs = parse_log("/tmp/log_filter.log", RE_DEBUG).expect("parse log");
        assert_eq!(debug_logs.len(), 0);
    }
}

#[test]
fn test_logger_assert_eq_with_msg() {
    lock_file!();

    let mut builder = split_error_file_logger("/tmp", "log_filter", Level::Trace);
    builder.dynamic = true;
    clear_test_files(&builder);
    setup_log(builder).expect("setup_log");

    let logger = LogFilter::new();
    logger_assert_eq!(logger, 2, 2, "my assert {}", "failed!");

    let debug_logs = parse_log("/tmp/log_filter.log", RE_DEBUG).expect("parse log");
    assert_eq!(debug_logs.len(), 0);
    let r = panic::catch_unwind(|| {
        logger_assert_eq!(logger, 2, 3, "my assert {}", "failed!");
    });
    assert!(r.is_err());
    let debug_logs = parse_log("/tmp/log_filter.log", RE_DEBUG).expect("parse log");
    // NOTE: with or without assertion msg is not totally the same
    let re = Regex::new(r"^assertion failed! expected.+left == right.+ actual.+2.+!=.+3").unwrap();
    assert!(re.is_match(&debug_logs[0][5]));
    // msg is in multiline text, not support to be parse
}

#[test]
fn test_logger_assert_eq_without_msg() {
    lock_file!();

    let mut builder = split_error_file_logger("/tmp", "log_filter", Level::Trace);
    builder.dynamic = true;
    clear_test_files(&builder);
    setup_log(builder).expect("setup_log");

    let logger = LogFilter::new();
    logger_assert_eq!(logger, 2, 2);

    let debug_logs = parse_log("/tmp/log_filter.log", RE_DEBUG).expect("parse log");
    assert_eq!(debug_logs.len(), 0);
    let r = panic::catch_unwind(|| {
        logger_assert_eq!(logger, 2, 3);
    });
    assert!(r.is_err());
    let debug_logs = parse_log("/tmp/log_filter.log", RE_DEBUG).expect("parse log");
    // NOTE: with or without assertion msg is not totally the same
    let re = Regex::new(r"^assertion failed! expected.+left == right.+ actual.+2.+!=.+3").unwrap();
    assert!(re.is_match(&debug_logs[0][5]));
}

#[test]
fn test_logger_debug_assert_eq_without_msg() {
    lock_file!();

    let mut builder = split_error_file_logger("/tmp", "log_filter", Level::Trace);
    builder.dynamic = true;
    clear_test_files(&builder);
    setup_log(builder).expect("setup_log");

    let logger = LogFilter::new();
    logger_debug_assert_eq!(logger, 1, 1);
    logger_debug_assert_eq!(logger, "hello", "hello", "assert failed!");
    logger_debug_assert_eq!(logger, "hello", "hello");
    let r = panic::catch_unwind(|| {
        logger_debug_assert_eq!(logger, 1, 2);
    });
    #[cfg(debug_assertions)]
    {
        assert!(r.is_err());
        let debug_logs = parse_log("/tmp/log_filter.log", RE_DEBUG).expect("parse log");
        // NOTE: with or without assertion msg is not totally the same
        let re =
            Regex::new(r"^assertion failed! expected.+left == right.+ actual.+1.+!=.+2").unwrap();
        assert!(re.is_match(&debug_logs[0][5]));
    }
    #[cfg(not(debug_assertions))]
    {
        // On release the debug_assert is completely bypassed
        assert!(r.is_ok());
        let debug_logs = parse_log("/tmp/log_filter.log", RE_DEBUG).expect("parse log");
        assert_eq!(debug_logs.len(), 0);
    }
}

#[test]
fn test_logger_debug_assert_eq_with_msg() {
    lock_file!();

    let mut builder = split_error_file_logger("/tmp", "log_filter", Level::Trace);
    builder.dynamic = true;
    clear_test_files(&builder);
    setup_log(builder).expect("setup_log");

    let logger = LogFilter::new();
    logger_debug_assert_eq!(logger, 1, 1);
    logger_debug_assert_eq!(logger, "hello", "hello", "assert failed!");
    logger_debug_assert_eq!(logger, "hello", "hello");
    let r = panic::catch_unwind(|| {
        logger_debug_assert_eq!(logger, 1, 2, "my assertion");
    });
    #[cfg(debug_assertions)]
    {
        assert!(r.is_err());
        let debug_logs = parse_log("/tmp/log_filter.log", RE_DEBUG).expect("parse log");
        // NOTE: with or without assertion msg is not totally the same
        let re =
            Regex::new(r"^assertion failed! expected.+left == right.+ actual.+1.+!=.+2").unwrap();
        assert!(re.is_match(&debug_logs[0][5]));
    }
    #[cfg(not(debug_assertions))]
    {
        // On release the debug_assert is completely bypassed
        assert!(r.is_ok());
        let debug_logs = parse_log("/tmp/log_filter.log", RE_DEBUG).expect("parse log");
        assert_eq!(debug_logs.len(), 0);
    }
}
