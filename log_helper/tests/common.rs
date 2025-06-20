use captains_log::Builder;
use std::fs::remove_file;

pub fn clear_test_files(builder: &Builder) {
    for sink in &builder.sinks {
        if let Some(file_path) = sink.get_file_path() {
            let _ = remove_file(file_path);
        }
    }
}

