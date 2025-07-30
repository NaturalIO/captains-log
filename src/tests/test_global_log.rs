use super::utils::*;
use crate::macros::*;
use log::*;
use regex::Regex;
use std::fs::*;
use std::panic;

use crate::{recipe, recipe::split_error_file_logger};

const RE_DEBUG: &str = r"^\[(.+)\]\[(\w+)\]\[(.+)\:(\d+)\] (.+)$";

const RE_ERROR: &str = r"^\[(.+)\]\[(\w+)\] (.+)$";

#[test]
fn test_global_log_console() {
    lock_file!();
    let mut builder = recipe::stderr_logger(Level::Debug);
    builder.dynamic = true;
    builder.build().expect("setup_log");
    debug!("test1 {}", "debug");
    info!("test2");
    error!("test3_error {}", "hahah");
}

#[test]
fn test_global_log_file() {
    lock_file!();

    let mut builder = split_error_file_logger("/tmp", "log_test", Level::Debug);
    builder.dynamic = true;
    clear_test_files(&builder);
    builder.build().expect("setup_log");
    debug!("test1 {}", "debug");
    info!("test2");
    error!("test3_error {}", "hahah");
    let debug_logs = parse_log("/tmp/log_test.log", RE_DEBUG).expect("parse log");
    assert_eq!(debug_logs.len(), 3);
    assert_eq!(debug_logs[0][2], "DEBUG");
    assert_eq!(debug_logs[0][3], "test_global_log.rs");
    assert_eq!(debug_logs[0][5], "test1 debug");
    assert_eq!(debug_logs[1][2], "INFO");
    assert_eq!(debug_logs[1][5], "test2");
    assert_eq!(debug_logs[2][5], "test3_error hahah");
    assert_eq!(debug_logs[2][2], "ERROR");

    let err_logs = parse_log("/tmp/log_test.log.wf", RE_ERROR).expect("parse log.wf");
    assert_eq!(err_logs.len(), 1);
    assert_eq!(err_logs[0][3], "test3_error hahah");
    assert_eq!(err_logs[0][2], "ERROR");

    std::fs::remove_file("/tmp/log_test.log").expect("remove log file");
    // log_test.log is removed, log_test.log.wf is kept.

    #[allow(static_mut_refs)]
    unsafe {
        libc::kill(std::process::id() as libc::c_int, signal_hook::consts::SIGUSR1);
    }
    std::thread::sleep(std::time::Duration::new(1, 0));
    info!("after reopen");
    info!("test4");
    warn!("test5 warn {}", 5);
    error!("test6 error");
    debug!("test7 debug");
    log_println!("test println a={} b={}", 1, 2);
    let debug_logs = parse_log("/tmp/log_test.log", RE_DEBUG).expect("parse log");
    assert_eq!(debug_logs.len(), 6);
    assert_eq!(debug_logs[0][5], "after reopen");
    assert_eq!(debug_logs[5][5], "test println a=1 b=2");
    let err_logs = parse_log("/tmp/log_test.log.wf", RE_ERROR).expect("parse log.wf");
    assert_eq!(err_logs.len(), 2);
}

#[test]
fn test_global_log_assert_without_msg() {
    lock_file!();

    let mut builder = split_error_file_logger("/tmp", "log_test", Level::Debug);
    builder.continue_when_panic = false;
    builder.dynamic = true;
    clear_test_files(&builder);
    builder.build().expect("setup_log");

    log_assert!(true);
    let debug_logs = parse_log("/tmp/log_test.log", RE_DEBUG).expect("parse log");
    assert_eq!(debug_logs.len(), 0);

    let r = panic::catch_unwind(|| {
        log_assert!(false);
    });
    assert!(r.is_err());
    let debug_logs = parse_log("/tmp/log_test.log", RE_DEBUG).expect("parse log");
    assert_eq!(debug_logs[0][5], "assertion failed: false");
}

#[test]
fn test_global_log_assert_with_msg() {
    lock_file!();

    let mut builder = split_error_file_logger("/tmp", "log_test", Level::Debug);
    builder.continue_when_panic = false;
    builder.dynamic = true;
    clear_test_files(&builder);
    builder.build().expect("setup_log");

    log_assert!(true);
    log_assert!(true, "log: assert failed");
    log_assert_eq!("a", "a", "log: assert failed!");

    let debug_logs = parse_log("/tmp/log_test.log", RE_DEBUG).expect("parse log");
    assert_eq!(debug_logs.len(), 0);

    let r = panic::catch_unwind(|| {
        log_assert!(false, "assert msg");
    });
    assert!(r.is_err());
    let debug_logs = parse_log("/tmp/log_test.log", RE_DEBUG).expect("parse log");
    assert_eq!(debug_logs[0][5], "assertion failed: assert msg");
}

#[test]
fn test_global_log_assert_eq_without_msg() {
    lock_file!();

    let mut builder = split_error_file_logger("/tmp", "log_test", Level::Debug);
    builder.continue_when_panic = false;
    builder.dynamic = true;
    clear_test_files(&builder);
    builder.build().expect("setup_log");

    log_assert_eq!("a", "a");

    let debug_logs = parse_log("/tmp/log_test.log", RE_DEBUG).expect("parse log");
    assert_eq!(debug_logs.len(), 0);

    let r = panic::catch_unwind(|| {
        log_assert_eq!("a", "b");
    });
    assert!(r.is_err());
    let debug_logs = parse_log("/tmp/log_test.log", RE_DEBUG).expect("parse log");
    // NOTE: with or without assertion msg is not totally the same
    let re = Regex::new(r"^assertion failed! expected.+left == right.+ actual.+a.+!=.+b").unwrap();
    assert!(re.is_match(&debug_logs[0][5]));
}

#[test]
fn test_global_log_assert_eq_with_msg() {
    lock_file!();

    let mut builder = split_error_file_logger("/tmp", "log_test", Level::Debug);
    builder.continue_when_panic = false;
    builder.dynamic = true;
    clear_test_files(&builder);
    builder.build().expect("setup_log");

    log_assert_eq!("a", "a", "msg");

    let debug_logs = parse_log("/tmp/log_test.log", RE_DEBUG).expect("parse log");
    assert_eq!(debug_logs.len(), 0);

    let r = panic::catch_unwind(|| {
        log_assert_eq!("a", "b", "msg");
    });
    assert!(r.is_err());
    let debug_logs = parse_log("/tmp/log_test.log", RE_DEBUG).expect("parse log");
    // NOTE: with or without assertion msg is not totally the same
    let re = Regex::new(r"^assertion failed! expected.+left == right.+ actual.+a.+!=.+b").unwrap();
    assert!(re.is_match(&debug_logs[0][5]));
    // msg is in multiline text, not support to be parse
}

#[test]
fn test_global_log_debug_assert_without_msg() {
    lock_file!();

    let mut builder = split_error_file_logger("/tmp", "log_test", Level::Debug);
    builder.continue_when_panic = false;
    builder.dynamic = true;
    clear_test_files(&builder);
    builder.build().expect("setup_log");

    log_debug_assert!(true);

    let debug_logs = parse_log("/tmp/log_test.log", RE_DEBUG).expect("parse log");
    assert_eq!(debug_logs.len(), 0);

    let r = panic::catch_unwind(|| {
        log_debug_assert!(false);
    });
    #[cfg(debug_assertions)]
    {
        assert!(r.is_err());
        let debug_logs = parse_log("/tmp/log_test.log", RE_DEBUG).expect("parse log");
        assert_eq!(debug_logs[0][5], "assertion failed: false");
    }
    #[cfg(not(debug_assertions))]
    {
        // On release the debug_assert is completely bypassed
        assert!(r.is_ok());
        let debug_logs = parse_log("/tmp/log_test.log", RE_DEBUG).expect("parse log");
        assert_eq!(debug_logs.len(), 0);
    }
}

#[test]
fn test_global_log_debug_assert_with_msg() {
    lock_file!();

    let mut builder = split_error_file_logger("/tmp", "log_test", Level::Debug);
    builder.continue_when_panic = false;
    builder.dynamic = true;
    clear_test_files(&builder);
    builder.build().expect("setup_log");

    log_debug_assert!(true, "msg");

    let debug_logs = parse_log("/tmp/log_test.log", RE_DEBUG).expect("parse log");
    assert_eq!(debug_logs.len(), 0);

    let r = panic::catch_unwind(|| {
        log_debug_assert!(false, "assert msg");
    });
    #[cfg(debug_assertions)]
    {
        assert!(r.is_err());
        let debug_logs = parse_log("/tmp/log_test.log", RE_DEBUG).expect("parse log");
        assert_eq!(debug_logs[0][5], "assertion failed: assert msg");
    }
    #[cfg(not(debug_assertions))]
    {
        // On release the debug_assert is completely bypassed
        assert!(r.is_ok());
        let debug_logs = parse_log("/tmp/log_test.log", RE_DEBUG).expect("parse log");
        assert_eq!(debug_logs.len(), 0);
    }
}

#[test]
fn test_global_log_debug_assert_eq_without_msg() {
    lock_file!();

    let mut builder = split_error_file_logger("/tmp", "log_test", Level::Debug);
    builder.continue_when_panic = false;
    builder.dynamic = true;
    clear_test_files(&builder);
    builder.build().expect("setup_log");

    log_debug_assert_eq!("a", "a");

    let debug_logs = parse_log("/tmp/log_test.log", RE_DEBUG).expect("parse log");
    assert_eq!(debug_logs.len(), 0);

    let r = panic::catch_unwind(|| {
        log_debug_assert_eq!("a", "b");
    });
    #[cfg(debug_assertions)]
    {
        assert!(r.is_err());
        let debug_logs = parse_log("/tmp/log_test.log", RE_DEBUG).expect("parse log");
        // NOTE: with or without assertion msg is not totally the same
        let re =
            Regex::new(r"^assertion failed! expected.+left == right.+ actual.+a.+!=.+b").unwrap();
        assert!(re.is_match(&debug_logs[0][5]));
    }
    #[cfg(not(debug_assertions))]
    {
        // On release the debug_assert is completely bypassed
        assert!(r.is_ok());
        let debug_logs = parse_log("/tmp/log_test.log", RE_DEBUG).expect("parse log");
        assert_eq!(debug_logs.len(), 0);
    }
}

#[test]
fn test_global_log_debug_assert_eq_with_msg() {
    lock_file!();

    let mut builder = split_error_file_logger("/tmp", "log_test", Level::Debug);
    builder.continue_when_panic = false;
    builder.dynamic = true;
    clear_test_files(&builder);
    builder.build().expect("setup_log");

    log_debug_assert_eq!("a", "a", "msg");

    let debug_logs = parse_log("/tmp/log_test.log", RE_DEBUG).expect("parse log");
    assert_eq!(debug_logs.len(), 0);

    let r = panic::catch_unwind(|| {
        log_debug_assert_eq!("a", "b", "msg");
    });
    #[cfg(debug_assertions)]
    {
        assert!(r.is_err());
        let debug_logs = parse_log("/tmp/log_test.log", RE_DEBUG).expect("parse log");
        // NOTE: with or without assertion msg is not totally the same
        let re =
            Regex::new(r"^assertion failed! expected.+left == right.+ actual.+a.+!=.+b").unwrap();
        assert!(re.is_match(&debug_logs[0][5]));
    }
    #[cfg(not(debug_assertions))]
    {
        // On release the debug_assert is completely bypassed
        assert!(r.is_ok());
        let debug_logs = parse_log("/tmp/log_test.log", RE_DEBUG).expect("parse log");
        assert_eq!(debug_logs.len(), 0);
    }
}
