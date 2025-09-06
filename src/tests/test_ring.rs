use super::utils::*;
use log::*;
use std::fs::OpenOptions;
use std::panic;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::thread;

#[test]
fn test_ringfile_assert() {
    lock_file!();
    crate::recipe::ring_file("/tmp/ring.log", 10240, Level::Debug, crate::signal_consts::SIGINT)
        .build()
        .expect("setup");
    let counter = Arc::new(AtomicUsize::new(0));
    let mut th_s = Vec::new();
    for _ in 0..4 {
        let _counter = counter.clone();
        th_s.push(thread::spawn(move || loop {
            let c = _counter.fetch_add(1, Ordering::Relaxed);
            if c == 100 {
                // NOTE: one of the thread panic, will not trigger unwind in the main thread.
                // But flush() called from panic hook will trigger the dumping of RingFile
                panic!("reach");
            } else if c > 100 {
                return;
            }
            debug!("count {}", c);
            std::hint::spin_loop();
        }));
    }
    for th in th_s {
        let _ = th.join();
    }
}
