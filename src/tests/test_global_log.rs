use std::fs::*;

use super::utils::*;
use crate::macros::*;
use log::*;

use crate::{recipe, recipe::split_error_file_logger};

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
}

#[test]
fn test_global_log_assert() {
    lock_file!();

    let mut builder = split_error_file_logger("/tmp", "log_test", Level::Debug);
    builder.dynamic = true;
    clear_test_files(&builder);
    builder.build().expect("setup_log");

    // Change the following condition to see the result
    log_assert!(true);
    log_assert!(true, "log: assert failed");
    log_debug_assert!(true, "log: assert failed");
    log_assert_eq!("a", "a", "log: assert failed!");
    log_debug_assert_eq!("a", "a", "log: assert failed!");
}
