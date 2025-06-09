use crate::{
    config::{LogConsole, LogFormat},
    log_impl::LoggerSinkTrait,
    time::Timer,
};
use log::{Level, Record};

pub struct LoggerSinkConsole {
    target_fd: libc::c_int,
    max_level: Level,
    formatter: LogFormat,
}

impl LoggerSinkConsole {
    pub fn new(config: &LogConsole) -> Self {
        Self {
            target_fd: config.target as i32,
            max_level: config.level,
            formatter: config.format.clone(),
        }
    }
}

impl LoggerSinkTrait for LoggerSinkConsole {
    fn reopen(&self) -> std::io::Result<()> {
        Ok(())
    }

    #[inline(always)]
    fn log(&self, now: &Timer, r: &Record) {
        if r.level() <= self.max_level {
            let buf = self.formatter.process(now, r);
            unsafe {
                let _ = libc::write(self.target_fd, buf.as_ptr() as *const libc::c_void, buf.len());
            }
        }
    }
}
