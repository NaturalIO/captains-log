use std::fs::*;
use super::utils::*;
use super::utils::lock_file;
use crate::{recipe::split_error_file_logger, setup_log, LogFilter};

use log::*;

use crate::macros::*;

#[test]
fn test_sub_logger() {
    lock_file!();

    let mut builder = split_error_file_logger("/tmp", "log_filter", Level::Trace);
    builder.force = true;
    clear_test_files(&builder);
    setup_log(builder);

    let logger = LogFilter::new(111);
    logger_trace!(logger, "hahaha {} {}", "hello", "world");
    logger_debug!(logger, "captain's log");
    logger.set_level(Level::Info);
    logger_debug!(logger, "expect to be filter!");
    logger_info!(logger, "Make it so");
    logger_error!(logger, "Fire phasers!");
}

#[test]
fn test_logger_assert() {
    lock_file!();

    let mut builder = split_error_file_logger("/tmp", "log_filter", Level::Trace);
    builder.force = true;
    clear_test_files(&builder);
    setup_log(builder);

    let logger = LogFilter::new(222);
    // Change the following condition to see the result
    logger_assert!(logger, true);
    logger_assert!(logger, true, "my assert {}", "failed!");
}

#[test]
fn test_logger_debug_assert_cond() {
    lock_file!();

    let mut builder = split_error_file_logger("/tmp", "log_filter", Level::Trace);
    builder.force = true;
    clear_test_files(&builder);
    setup_log(builder);
    let logger = LogFilter::new(333);
    // Change the following condition to see the result
    logger_assert!(logger, true);
    logger_assert!(logger, true, "my assert {}", "failed!");
}

#[test]
fn test_logger_assert_eq() {
    lock_file!();

    let mut builder = split_error_file_logger("/tmp", "log_filter", Level::Trace);
    builder.force = true;
    clear_test_files(&builder);
    setup_log(builder);

    let logger = LogFilter::new(444);
    // Change the following condition to see the result
    logger_assert_eq!(logger, "hello", "hello", "hello world failed");
    logger_assert_eq!(logger, 2, 2, "my assert {}", "failed!");
}

#[test]
fn test_logger_debug_assert_eq() {
    lock_file!();

    let mut builder = split_error_file_logger("/tmp", "log_filter", Level::Trace);
    builder.force = true;
    clear_test_files(&builder);
    setup_log(builder);

    let logger = LogFilter::new(555);
    logger_debug_assert_eq!(logger, 1, 1);
    logger_debug_assert_eq!(logger, "hello", "hello", "assert failed!");
    logger_debug_assert_eq!(logger, "hello", "hello");
}
