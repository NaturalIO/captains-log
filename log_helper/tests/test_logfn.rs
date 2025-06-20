use captains_log::{recipe};
use log::*;
use captains_log_helper::logfn;

mod common;

#[test]
fn test_log_fn() {
    let builder = recipe::raw_file_logger("/tmp", "log_test", log::Level::Debug).test();
    common::clear_test_files(&builder);
    builder.build().expect("setup_log");

    #[logfn]
    fn foo() {
        info!("foo");
        bar(1, "bar arg");
    }

    #[logfn(warn)]
    fn bar(a: i32, s: &str) {
        info!("bar");
    }

    foo();
}
