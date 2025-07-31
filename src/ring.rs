use crate::{
    config::{LogFormat, SinkConfigBuild, SinkConfigTrait},
    log_impl::{LogSink, LogSinkTrait},
    time::Timer,
};
use log::*;
use ring_file::*;

use std::cell::UnsafeCell;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::mem::transmute;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Hash)]
pub struct LogRingFile {
    pub file_path: Box<Path>,
    pub level: Level,
    pub format: LogFormat,
    /// 0 < buf_size < i32::MAX
    pub buf_size: i32,
}

impl LogRingFile {
    pub fn new<P: Into<PathBuf>>(
        file_path: P, buf_size: i32, max_level: Level, format: LogFormat,
    ) -> Self {
        assert!(buf_size > 0);
        Self { buf_size, file_path: file_path.into().into_boxed_path(), level: max_level, format }
    }
}

impl SinkConfigBuild for LogRingFile {
    fn build(&self) -> LogSink {
        LogSink::RingFile(LogSinkRingFile::new(self))
    }
}

impl SinkConfigTrait for LogRingFile {
    fn get_level(&self) -> Level {
        self.level
    }

    fn get_file_path(&self) -> Option<Box<Path>> {
        Some(self.file_path.clone())
    }

    fn write_hash(&self, hasher: &mut Box<dyn Hasher>) {
        self.hash(hasher);
        hasher.write(b"LogRingFile");
    }
}

pub(crate) struct LogSinkRingFile {
    max_level: Level,
    inner: UnsafeCell<RingFile>,
    formatter: LogFormat,
    /// In order to be fast, use a spin lock instead of Mutex
    locked: AtomicBool,
}

unsafe impl Send for LogSinkRingFile {}
unsafe impl Sync for LogSinkRingFile {}

impl LogSinkRingFile {
    fn new(config: &LogRingFile) -> Self {
        Self {
            max_level: config.level,
            inner: UnsafeCell::new(RingFile::new(config.buf_size, config.file_path.to_path_buf())),
            formatter: config.format.clone(),
            locked: AtomicBool::new(false),
        }
    }

    #[inline(always)]
    fn get_inner(&self) -> &RingFile {
        unsafe { transmute(self.inner.get()) }
    }

    #[inline(always)]
    fn get_inner_mut(&self) -> &mut RingFile {
        unsafe { transmute(self.inner.get()) }
    }
}

impl LogSinkTrait for LogSinkRingFile {
    fn reopen(&self) -> std::io::Result<()> {
        println!("RingFile: start dumpping");
        if let Err(e) = self.get_inner().dump() {
            println!("RingFile: dump error {:?}", e);
            Err(e)
        } else {
            println!("RingFile: dump complete");
            Ok(())
        }
    }

    #[inline(always)]
    fn log(&self, now: &Timer, r: &Record) {
        if r.level() <= self.max_level {
            let buf = self.formatter.process(now, r);
            while self
                .locked
                .compare_exchange_weak(false, true, Ordering::SeqCst, Ordering::Relaxed)
                .is_err()
            {
                std::hint::spin_loop();
            }
            let _ = self.get_inner_mut().write_all(buf.as_bytes());
            self.locked.store(false, Ordering::Release);
        }
    }

    #[inline(always)]
    fn flush(&self) {}
}
