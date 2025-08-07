use crate::{buf_file_impl::LogSinkBufFile, console_impl::LogSinkConsole, file_impl::LogSinkFile};
use crate::{config::Builder, time::Timer};
use arc_swap::ArcSwap;
use backtrace::Backtrace;
use signal_hook::iterator::Signals;
use std::cell::UnsafeCell;
use std::mem::transmute;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use std::thread;

#[enum_dispatch]
pub(crate) trait LogSinkTrait {
    fn open(&self) -> std::io::Result<()>;

    fn reopen(&self) -> std::io::Result<()>;

    fn log(&self, now: &Timer, r: &log::Record);

    fn flush(&self);
}

#[enum_dispatch(LogSinkTrait)]
pub enum LogSink {
    File(LogSinkFile),
    BufFile(LogSinkBufFile),
    Console(LogSinkConsole),
    #[cfg(feature = "syslog")]
    Syslog(crate::syslog::LogSinkSyslog),
    #[cfg(feature = "ringfile")]
    RingFile(crate::ring::LogSinkRingFile),
}

struct GlobalLoggerStatic {
    logger: UnsafeCell<GlobalLogger>,
    lock: AtomicBool,
}

struct GlobalLoggerGuard<'a>(&'a GlobalLoggerStatic);

impl Drop for GlobalLoggerGuard<'_> {
    fn drop(&mut self) {
        self.0.unlock();
    }
}

impl GlobalLoggerStatic {
    const fn new() -> Self {
        Self {
            logger: UnsafeCell::new(GlobalLogger {
                config_checksum: AtomicU64::new(0),
                inner: None,
                signal_listener: AtomicBool::new(false),
            }),
            lock: AtomicBool::new(false),
        }
    }

    fn get_logger_mut(&self) -> &mut GlobalLogger {
        unsafe { transmute(self.logger.get()) }
    }

    fn get_logger(&self) -> &GlobalLogger {
        unsafe { transmute(self.logger.get()) }
    }

    fn lock<'a>(&'a self) -> GlobalLoggerGuard<'a> {
        while self
            .lock
            .compare_exchange_weak(false, true, Ordering::SeqCst, Ordering::Relaxed)
            .is_err()
        {
            // Normally this does not contend, if your test does not run concurrently.
            std::thread::yield_now();
        }
        GlobalLoggerGuard(self)
    }

    fn unlock(&self) {
        self.lock.store(false, Ordering::SeqCst);
    }

    /// Return Ok(false) when reinit, Ok(true) when first init, Err for error
    fn try_setup(&self, builder: &Builder) -> Result<bool, ()> {
        let _guard = self.lock();
        let res = { self.get_logger().check_the_same(builder) };
        match res {
            Some(true) => {
                if let Err(e) = self.get_logger().open() {
                    eprintln!("failed to open log sink: {:?}", e);
                    return Err(());
                }
                return Ok(false);
            }
            Some(false) => {
                if !builder.dynamic {
                    panic_or_error();
                    return Err(());
                }
                let res = self.get_logger().reinit(builder);
                res?;
                // reset the log level
                log::set_max_level(builder.get_max_level());
                return Ok(false);
            }
            None => {
                let res = { self.get_logger_mut().init(builder) };
                res?;
                return Ok(true);
            }
        }
    }
}

unsafe impl Send for GlobalLoggerStatic {}
unsafe impl Sync for GlobalLoggerStatic {}

/// Initialize global logger from Builder
///
/// **NOTE**: You can call this function multiple times when **builder.dynamic=true**,
/// but **cannot mixed used captains_log with other logger implement**, because log::set_logger()
/// cannot be called twice.
pub fn setup_log(builder: Builder) -> Result<(), ()> {
    if let Ok(true) = GLOBAL_LOGGER.try_setup(&builder) {
        let logger = GLOBAL_LOGGER.get_logger();
        // Set logger can only be called once
        if let Err(e) = log::set_logger(logger) {
            eprintln!("log::set_logger return error: {:?}", e);
            return Err(());
        }
        log::set_max_level(builder.get_max_level());
        // panic hook can be set multiple times
        if builder.continue_when_panic {
            std::panic::set_hook(Box::new(panic_no_exit_hook));
        } else {
            std::panic::set_hook(Box::new(panic_and_exit_hook));
        }
        let signals = builder.rotation_signals.clone();
        if signals.len() > 0 {
            if false == logger.signal_listener.swap(true, Ordering::SeqCst) {
                thread::spawn(move || {
                    GLOBAL_LOGGER.get_logger().listener_for_signal(signals);
                });
            }
        }
    }
    Ok(())
}

/// Global static structure to hold the logger
struct GlobalLogger {
    /// checksum for config comparison
    config_checksum: AtomicU64,
    /// Global static needs initialization when declaring,
    /// default to be empty
    inner: Option<LoggerInner>,
    signal_listener: AtomicBool,
}

enum LoggerInner {
    Once(Vec<LogSink>),
    // using ArcSwap has more cost
    Dyn(ArcSwap<Vec<LogSink>>),
}

#[inline(always)]
fn panic_or_error() {
    #[cfg(debug_assertions)]
    {
        panic!("GlobalLogger cannot be initialized twice on dynamic==false");
    }
    #[cfg(not(debug_assertions))]
    {
        eprintln!("GlobalLogger cannot be initialized twice on dynamic==false");
    }
}

impl LoggerInner {
    #[allow(dead_code)]
    fn set(&self, sinks: Vec<LogSink>) {
        match &self {
            Self::Once(_) => {
                panic_or_error();
            }
            Self::Dyn(d) => {
                d.store(Arc::new(sinks));
            }
        }
    }
}

impl GlobalLogger {
    fn listener_for_signal(&self, signals: Vec<i32>) {
        println!("signal_listener started");
        let mut signals = Signals::new(&signals).unwrap();
        for __sig in signals.forever() {
            let _ = self.reopen();
        }
        println!("signal_listener exit");
    }

    /// On program/test Initialize
    fn open(&self) -> std::io::Result<()> {
        if let Some(inner) = self.inner.as_ref() {
            match &inner {
                LoggerInner::Once(inner) => {
                    for sink in inner.iter() {
                        sink.open()?;
                    }
                }
                LoggerInner::Dyn(inner) => {
                    let sinks = inner.load();
                    for sink in sinks.iter() {
                        sink.open()?;
                    }
                }
            }
        }
        println!("log sinks opened");
        Ok(())
    }

    /// On signal to reopen file.
    pub fn reopen(&self) -> std::io::Result<()> {
        if let Some(inner) = self.inner.as_ref() {
            match &inner {
                LoggerInner::Once(inner) => {
                    for sink in inner.iter() {
                        sink.reopen()?;
                    }
                }
                LoggerInner::Dyn(inner) => {
                    let sinks = inner.load();
                    for sink in sinks.iter() {
                        sink.reopen()?;
                    }
                }
            }
        }
        println!("log sinks re-opened");
        Ok(())
    }

    /// Return Some(true) to skip, Some(false) to reinit, None to init
    fn check_the_same(&self, builder: &Builder) -> Option<bool> {
        if self.inner.is_some() {
            return Some(self.config_checksum.load(Ordering::Acquire) == builder.cal_checksum());
        }
        None
    }

    /// Re-configure the logger sink
    fn reinit(&self, builder: &Builder) -> Result<(), ()> {
        let sinks = builder.build_sinks()?;
        if let Some(inner) = self.inner.as_ref() {
            inner.set(sinks);
            self.config_checksum.store(builder.cal_checksum(), Ordering::Release);
        } else {
            unreachable!();
        }
        Ok(())
    }

    #[allow(dead_code)]
    fn init(&mut self, builder: &Builder) -> Result<(), ()> {
        let sinks = builder.build_sinks()?;
        assert!(self.inner.is_none());
        if builder.dynamic {
            self.inner.replace(LoggerInner::Dyn(ArcSwap::new(Arc::new(sinks))));
        } else {
            self.inner.replace(LoggerInner::Once(sinks));
        }
        self.config_checksum.store(builder.cal_checksum(), Ordering::Release);
        Ok(())
    }
}

impl log::Log for GlobalLogger {
    #[inline(always)]
    fn enabled(&self, _m: &log::Metadata) -> bool {
        true
    }

    #[inline(always)]
    fn log(&self, r: &log::Record) {
        let now = Timer::new();
        if let Some(inner) = self.inner.as_ref() {
            match &inner {
                LoggerInner::Once(inner) => {
                    for sink in inner.iter() {
                        sink.log(&now, r);
                    }
                }
                LoggerInner::Dyn(inner) => {
                    let sinks = inner.load();
                    for sink in sinks.iter() {
                        sink.log(&now, r);
                    }
                }
            }
        }
    }

    /// Can be call manually on program shutdown (If you have a buffered log sink)
    ///
    /// # Example
    ///
    /// ``` rust
    /// log::logger().flush();
    /// ```
    fn flush(&self) {
        if let Some(inner) = self.inner.as_ref() {
            match &inner {
                LoggerInner::Once(inner) => {
                    for sink in inner.iter() {
                        sink.flush();
                    }
                }
                LoggerInner::Dyn(inner) => {
                    let sinks = inner.load();
                    for sink in sinks.iter() {
                        sink.flush();
                    }
                }
            }
        }
    }
}

static GLOBAL_LOGGER: GlobalLoggerStatic = GlobalLoggerStatic::new();

/// log handle for panic hook
#[doc(hidden)]
pub fn log_panic(info: &std::panic::PanicHookInfo) {
    let bt = Backtrace::new();
    let mut record = log::Record::builder();
    record.level(log::Level::Error);
    if let Some(loc) = info.location() {
        record.file(Some(loc.file())).line(Some(loc.line()));
    }
    log::logger().log(&record.args(format_args!("panic occur: {}\ntrace: {:?}", info, bt)).build());
    eprint!("panic occur: {} at {:?}\ntrace: {:?}", info, info.location(), bt);
}

#[inline(always)]
fn panic_and_exit_hook(info: &std::panic::PanicHookInfo) {
    log_panic(info);
    log::logger().flush();
    let msg = format!("{}", info).to_string();
    std::panic::resume_unwind(Box::new(msg));
}

#[inline(always)]
fn panic_no_exit_hook(info: &std::panic::PanicHookInfo) {
    log_panic(info);
    eprint!("not debug version, so don't exit process");
    log::logger().flush();
}
