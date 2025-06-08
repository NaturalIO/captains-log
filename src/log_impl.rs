use std::thread;
use backtrace::Backtrace;
use lazy_static::lazy_static;
use log::*;
use parking_lot::Mutex;
use signal_hook::iterator::Signals;
use crate::{
    config::Builder,
    file_impl::LoggerSinkFile,
    time::Timer,
};

/*
 * This is logger that support multi-thread appending write without lock.
 * Because Log trait prevent internal mutability, File has Sync & Send but need mut to write, and
 * RefCell has !Send.  So the only way to achieve lock-free is to use unsafe libc call.
*/

#[enum_dispatch]
pub(crate) trait LoggerSinkTrait {

    fn reopen(&self) -> std::io::Result<()>;

    fn log(&self, now: &Timer, r: &Record);
}

#[enum_dispatch(LoggerSinkTrait)]
pub enum LoggerSink{
    File(LoggerSinkFile),
}

pub struct GlobalLogger {
    sinks: Option<Vec<LoggerSink>>, // Global static needs initialization when declaring, so we give it a wrapper struct with empty internal
}


impl GlobalLogger {
    pub fn reopen(&mut self) -> std::io::Result<()> {
        if let Some(sinks) = self.sinks.as_ref() {
            for sink in sinks {
                sink.reopen()?;
            }
        }
        Ok(())
    }

    fn init(&mut self, builder: &Builder) -> std::io::Result<bool> {
        if !builder.force || self.sinks.is_some() {
            return Ok(false);
        }
        let mut sinks = Vec::new();
        for config in &builder.sinks {
            let logger_sink = config.build();
            logger_sink.reopen()?;
            sinks.push(logger_sink);
        }
        self.sinks.replace(sinks);

        let _ = unsafe { set_logger(std::mem::transmute::<&Self, &'static Self>(self)) };
        Ok(true)
    }
}

impl Log for GlobalLogger {
    fn enabled(&self, _m: &Metadata) -> bool {
        true
    }

    fn log(&self, r: &Record) {
        let now = Timer::new();
        if let Some(sinks) = self.sinks.as_ref() {
            for sink in sinks {
                sink.log(&now, r);
            }
        }
    }

    fn flush(&self) {}
}

lazy_static! {
    static ref GLOBAL_LOGGER: Mutex<GlobalLogger> = Mutex::new(GlobalLogger { sinks: None });
}

/// log handle for panic hook
#[doc(hidden)]
pub fn log_panic(info: &std::panic::PanicHookInfo) {
    let bt = Backtrace::new();
    let mut record = log::Record::builder();
    record.level(log::Level::Error);
    if let Some(loc) = info.location() {
        record.file(Some(loc.file())).line(Some(loc.line()));
    }
    log::logger().log(
        &record
            .args(format_args!("panic occur: {}\ntrace: {:?}", info, bt))
            .build(),
    );
    eprint!(
        "panic occur: {} at {:?}\ntrace: {:?}",
        info,
        info.location(),
        bt
    );
}

fn panic_and_exit_hook(info: &std::panic::PanicHookInfo) {
    log_panic(info);
    std::process::exit(exitcode::IOERR);
}

fn panic_no_exit_hook(info: &std::panic::PanicHookInfo) {
    log_panic(info);
    eprint!("not debug version, so don't exit process");
}

/// Initialize global logger from Builder
pub fn setup_log(builder: Builder) {
    {
        let mut global_logger = GLOBAL_LOGGER.lock();

        match global_logger.init(&builder) {
            Err(e) => {
                println!("Initialize logger failed: {:?}", e);
                return;
            }
            Ok(false) => return,
            Ok(true) => {}
        }
        set_max_level(builder.get_max_level());
        if builder.continue_when_panic {
            std::panic::set_hook(Box::new(panic_no_exit_hook));
        } else {
            std::panic::set_hook(Box::new(panic_and_exit_hook));
        }
    }
    if builder.rotation_signals.len() > 0 {
        let signals = builder.rotation_signals.clone();
        thread::spawn(move || {
            let mut signals = Signals::new(&signals).unwrap();
            for __sig in signals.forever() {
                let mut global_logger = GLOBAL_LOGGER.lock();
                let _ = global_logger.reopen();
            }
        });
    }
}
