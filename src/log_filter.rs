use std::{
    fmt,
    io::Write,
    str,
    sync::atomic::{AtomicUsize, Ordering},
};

use log::{kv::*, *};

pub struct LogFilter {
    req_id: u64,
    req_str: [u8; 16],
    max_level: AtomicUsize,
}

impl Clone for LogFilter {
    fn clone(&self) -> Self {
        Self {
            req_id: self.req_id,
            req_str: self.req_str.clone(),
            max_level: AtomicUsize::new(self.get_level()),
        }
    }
}

impl LogFilter {
    pub fn new(req_id: u64) -> Self {
        let mut s = Self {
            req_id,
            max_level: AtomicUsize::new(Level::Trace as usize),
            req_str: [0u8; 16],
        };
        write!(&mut s.req_str[..], "{:016x}", req_id).expect("to hex");
        s
    }

    #[inline]
    pub fn set_level(&self, level: Level) {
        self.max_level.store(level as usize, Ordering::Relaxed);
    }

    #[inline]
    pub fn get_level(&self) -> usize {
        self.max_level.load(Ordering::Relaxed)
    }

    #[inline(always)]
    pub fn _private_api_log(
        &self,
        args: fmt::Arguments,
        level: Level,
        &(target, module_path, file, line): &(&str, &str, &str, u32),
    ) {
        let record = RecordBuilder::new()
            .level(level)
            .target(target)
            .module_path(Some(module_path))
            .file(Some(file))
            .line(Some(line))
            .key_values(&self)
            .args(args)
            .build();
        logger().log(&record);
    }
}

impl log::kv::Source for LogFilter {
    #[inline(always)]
    fn visit<'kvs>(&'kvs self, visitor: &mut dyn Visitor<'kvs>) -> Result<(), Error> {
        visitor.visit_pair("req_id".to_key(), unsafe {
            str::from_utf8_unchecked(&self.req_str).into()
        })
    }

    #[inline(always)]
    fn get<'a>(&'a self, key: Key) -> Option<Value<'a>> {
        if self.req_id != 0 && key.as_ref() == "req_id" {
            return Some(unsafe { str::from_utf8_unchecked(&self.req_str).into() });
        }
        return None;
    }

    #[inline(always)]
    fn count(&self) -> usize {
        1
    }
}
