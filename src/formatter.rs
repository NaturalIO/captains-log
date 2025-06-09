use std::*;

use log::{kv::Key, *};

use crate::time::Timer;

pub struct TimeFormatter<'a> {
    pub now: &'a Timer,
    pub fmt_str: &'a String,
}

impl<'a> TimeFormatter<'a> {
    #[inline(always)]
    fn time_str(&self) -> String {
        self.now.format(&self.fmt_str).to_string()
    }
}

pub struct FormatRecord<'a> {
    pub record: &'a Record<'a>,
    pub time: TimeFormatter<'a>,
}

impl<'a> FormatRecord<'a> {
    #[inline(always)]
    pub fn file(&self) -> &str {
        basename(self.record.file().unwrap_or("<none>"))
    }

    #[inline(always)]
    pub fn line(&self) -> u32 {
        self.record.line().unwrap_or(0)
    }

    #[inline(always)]
    pub fn time(&self) -> String {
        self.time.time_str()
    }

    #[inline(always)]
    pub fn key(&self, key: &str) -> String {
        let source = self.record.key_values();
        if let Some(v) = source.get(Key::from_str(key)) {
            return format!(" ({})", v).to_string();
        } else {
            return "".to_string();
        }
    }

    #[inline(always)]
    pub fn level(&self) -> Level {
        self.record.level()
    }

    #[inline(always)]
    pub fn msg(&self) -> &'a fmt::Arguments<'a> {
        self.record.args()
    }
}

fn basename(path: &str) -> &str {
    let res = path.rfind('/');
    match res {
        Some(idx) => path.get(idx + 1..).unwrap_or(path),
        None => path,
    }
}
