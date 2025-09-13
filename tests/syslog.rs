use captains_log::*;

#[cfg(feature = "syslog")]
#[test]
fn test_syslog() {
    let _ = recipe::syslog_local(Level::Debug).test().build().expect("setup");
    info!("begin syslog test");
    for _ in 0..20 {
        trace!("test syslog trace");
        debug!("test syslog debug");
        info!("test syslog info");
        warn!("test syslog warn");
        error!("test syslog error");
        println!("sleep");
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
