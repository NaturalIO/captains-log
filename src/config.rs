use log::{Level, LevelFilter};
use std::path::Path;
use crate::{
    log_impl::LoggerSink,
    formatter::LogFormat,
    file_impl::LoggerSinkFile,
};

#[derive(Default)]
pub struct Builder {
    pub force: bool,
    pub rotation_signals: Vec<i32>,
    pub panic: bool,
    pub sinks: Vec<Box<dyn SinkConfigTrait>>
}


impl Builder {

    pub fn new() -> Self{
        Self::default()
    }

    pub fn signal(mut self, signal: i32) -> Self {
        self.rotation_signals.push(signal);
        self
    }

    pub fn file(mut self, config: LogFile) -> Self {
        self.sinks.push(Box::new(config));
        self
    }

    pub fn get_max_level(&self) -> LevelFilter {
        let mut max_level = Level::Error;
        for sink in &self.sinks {
            let level = sink.get_level();
            if level > max_level {
                max_level = level;
            }
        }
        return max_level.to_level_filter();
    }
}

pub trait SinkConfigTrait {

    fn get_level(&self) -> Level;
    fn get_file_path(&self) -> Option<Box<Path>>;
    fn build(&self) -> LoggerSink;
}

pub struct LogFile {
    pub dir: String,
    pub level: Level,
    pub name: String,
    pub format: LogFormat,
    pub file_path: Box<Path>,
}

impl LogFile {

    pub fn new(dir: &str, name: &str, level: Level, format: LogFormat) -> Self {
        let file_path = Path::new(dir).join(Path::new(name)).into_boxed_path();
        Self{
            dir: dir.to_string(),
            name: name.to_string(),
            level,
            format,
            file_path,
        }
    }
}

impl SinkConfigTrait for LogFile {

    fn get_level(&self) -> Level {
        self.level
    }

    fn get_file_path(&self) -> Option<Box<Path>> {
        Some(self.file_path.clone())
    }

    fn build(&self) -> LoggerSink {
        LoggerSink::File(LoggerSinkFile::new(self))
    }
}
