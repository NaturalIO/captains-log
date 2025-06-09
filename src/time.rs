use chrono::{DateTime, Local};

pub struct Timer(DateTime<Local>);

impl std::ops::Deref for Timer {
    type Target = DateTime<Local>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Timer {
    pub(crate) fn new() -> Self {
        return Self(Local::now());
    }
}
