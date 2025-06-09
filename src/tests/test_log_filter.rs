use super::utils::lock_file;
use super::utils::*;
use crate::recipe;
use crate::{recipe::split_error_file_logger, setup_log, FormatRecord, LogFilter, LogFilterKV};
use std::fs::*;

use log::*;

use crate::macros::*;

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
    logger_error!(logger, "Fire phasers!");
}

#[test]
fn test_logger_filter_kv() {
    lock_file!();

    fn debug_format_req_id_f(r: FormatRecord) -> String {
        let time = r.time();
        let level = r.level();
        let file = r.file();
        let line = r.line();
        let msg = r.msg();
        let req_id = r.key("req_id");
        format!("[{time}][{level}][{file}:{line}] {msg}{req_id}\n").to_string()
    }
    let mut builder = recipe::file_logger_custom(
        "/tmp",
        "log_filter",
        Level::Debug,
        recipe::DEFAULT_TIME,
        debug_format_req_id_f,
    );
    builder.dynamic = true;
    clear_test_files(&builder);

    builder.build().expect("setup_log");
    let logger = LogFilterKV::new("req_id", format!("{:016x}", 123).to_string());
    logger_trace!(logger, "trace should be filtered");
    logger_debug!(logger, "captain's log");
}

#[test]
fn test_logger_assert() {
    lock_file!();

    let mut builder = split_error_file_logger("/tmp", "log_filter", Level::Trace);
    builder.dynamic = true;
    clear_test_files(&builder);
    setup_log(builder).expect("setup_log");

    let logger = LogFilter::new();
    // Change the following condition to see the result
    logger_assert!(logger, true);
    logger_assert!(logger, true, "my assert {}", "failed!");
}

#[test]
fn test_logger_debug_assert_cond() {
    lock_file!();

    let mut builder = split_error_file_logger("/tmp", "log_filter", Level::Trace);
    builder.dynamic = true;
    clear_test_files(&builder);
    setup_log(builder).expect("setup_log");
    let logger = LogFilter::new();
    // Change the following condition to see the result
    logger_assert!(logger, true);
    logger_assert!(logger, true, "my assert {}", "failed!");
}

#[test]
fn test_logger_assert_eq() {
    lock_file!();

    let mut builder = split_error_file_logger("/tmp", "log_filter", Level::Trace);
    builder.dynamic = true;
    clear_test_files(&builder);
    setup_log(builder).expect("setup_log");

    let logger = LogFilter::new();
    // Change the following condition to see the result
    logger_assert_eq!(logger, "hello", "hello", "hello world failed");
    logger_assert_eq!(logger, 2, 2, "my assert {}", "failed!");
}

#[test]
fn test_logger_debug_assert_eq() {
    lock_file!();

    let mut builder = split_error_file_logger("/tmp", "log_filter", Level::Trace);
    builder.dynamic = true;
    clear_test_files(&builder);
    setup_log(builder).expect("setup_log");

    let logger = LogFilter::new();
    logger_debug_assert_eq!(logger, 1, 1);
    logger_debug_assert_eq!(logger, "hello", "hello", "assert failed!");
    logger_debug_assert_eq!(logger, "hello", "hello");
}
