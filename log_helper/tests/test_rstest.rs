use rstest::*;
use log::*;
use captains_log::*;
use captains_log_helper::logfn;
use std::sync::Once;

static INIT: Once = Once::new();

mod common;

#[fixture]
fn setup() {
    INIT.call_once(|| {
        let builder = recipe::raw_file_logger("/tmp", "log_rstest", log::Level::Debug).test();
        common::clear_test_files(&builder);
        builder.build().expect("setup_log");
    });
}

#[logfn]
#[rstest(file_size, case(0), case(1))]
fn test_rstest_case_inline(setup: (), file_size: usize) {
    info!("do something1111");
}

#[logfn]
#[rstest]
#[case(0)]
#[case(1)]
fn test_rstest_case_separate(setup: (), #[case] file_size: usize) {
    info!("do something1112");
}

#[logfn]
#[rstest]
fn test_rstest_bar(setup: ()) {
    info!("do something222");
}

#[tokio::test]
#[logfn]
#[rstest]
async fn test_rstest_async(setup: ()) {
    info!("something333")
}
