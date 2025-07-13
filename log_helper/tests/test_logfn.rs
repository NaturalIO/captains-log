use captains_log::{recipe};
use log::*;
use captains_log_helper::logfn;
use std::time::Duration;
use async_trait::async_trait;

mod common;

fn setup_log() {
    let builder = recipe::raw_file_logger("/tmp", "log_test", log::Level::Debug).test();
    common::clear_test_files(&builder);
    builder.build().expect("setup_log");
}

#[test]
fn test_log_fn() {
    setup_log();

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


#[tokio::test]
async fn test_log_async_fn() {
    setup_log();

    #[logfn]
    async fn foo() {
        info!("foo");
        tokio::time::sleep(Duration::from_secs(1)).await;
        bar(1, "bar arg");
    }

    #[logfn(warn)]
    fn bar(a: i32, s: &str) {
        info!("bar");
    }
    foo().await;
}

#[tokio::test]
async fn test_log_async_trait() {
    setup_log();

    #[async_trait]
    trait MyTrait {

        #[logfn]
        async fn foo_tr(&self) {
            info!("inside foo");
        }

        fn bar_tr(&self);
    }

    pub struct Obj ();

    #[async_trait]
    impl MyTrait for Obj {

        #[logfn]
        fn bar_tr(&self) {
            info!("inside bar");
        }
    }

    let o = Obj();
    o.foo_tr().await;
    o.bar_tr();
}
