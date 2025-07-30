use super::utils::*;
use crate::*;
use log::*;
use rstest::*;
use std::fs::*;
use std::path::Path;
use std::thread;
use std::time::Duration;

const TEST_DIR: &'static str = "/tmp/captains_log/";

const TEST_DIR_OLD: &'static str = "/tmp/captains_log/old";

fn read_files<P: AsRef<Path>>(dir: P) -> Vec<DirEntry> {
    read_dir(dir).unwrap().filter_map(|entry| entry.ok()).collect()
}

#[rstest]
#[case(Some(3))]
#[case(Some(2))]
#[case(Some(1))]
#[case(None)]
fn test_buffered_file_rotation_by_size(#[case] max_files: Option<usize>) {
    lock_file!();
    let _ = remove_dir_all(TEST_DIR);
    let rotation = Rotation::by_size(1024 * 4 * 2, max_files);
    let base_path = Path::new(TEST_DIR).join("rotation_test.log");
    recipe::buffered_rotated_file_logger(base_path, Level::Debug, rotation)
        .test()
        .build()
        .expect("setup");

    for _i in 0..1000 {
        info!("test {}", _i);
    }
    logger().flush();
    thread::sleep(Duration::from_secs(1));
    let files = read_files(TEST_DIR);
    if let Some(_max_files) = max_files {
        assert_eq!(files.len(), _max_files + 1);
    } else {
        assert_eq!(files.len(), 6);
    }
}

#[rstest]
#[case(Some(3))]
#[case(Some(2))]
#[case(Some(1))]
#[case(None)]
fn test_buffered_file_rotation_by_size_compress(#[case] max_files: Option<usize>) {
    lock_file!();
    let _ = remove_dir_all(TEST_DIR);
    let rotation = Rotation::by_size(1024 * 4 * 2, max_files).compress_exclude(1);
    let base_path = Path::new(TEST_DIR).join("rotation_test.log");
    recipe::buffered_rotated_file_logger(base_path, Level::Debug, rotation)
        .test()
        .build()
        .expect("setup");

    for _i in 0..1000 {
        info!("test {}", _i);
    }
    logger().flush();
    // It seams like compress is not finished ?
    thread::sleep(Duration::from_secs(1));
    let files = read_files(TEST_DIR);
    if let Some(_max_files) = max_files {
        assert_eq!(files.len(), _max_files + 1);
        let files_len = files.len();
        if files.len() > 2 {
            let compressed: Vec<DirEntry> = files
                .into_iter()
                .filter(|e| e.file_name().to_string_lossy().ends_with(".gz"))
                .collect();
            assert_eq!(compressed.len() + 1 + 1, files_len); // + uncompressed + current
        }
    } else {
        assert_eq!(files.len(), 6);
    }
}

#[rstest]
#[case(Some(3))]
#[case(Some(2))]
#[case(Some(1))]
#[case(None)]
fn test_buffered_file_rotation_by_size_archive_dir(#[case] max_files: Option<usize>) {
    lock_file!();
    let _ = remove_dir_all(TEST_DIR);
    let rotation = Rotation::by_size(1024 * 4 * 2, max_files).archive_dir(TEST_DIR_OLD);
    let base_path = Path::new(TEST_DIR).join("rotation_test.log");
    recipe::buffered_rotated_file_logger(base_path, Level::Debug, rotation)
        .test()
        .build()
        .expect("setup");

    for _i in 0..1000 {
        info!("test {}", _i);
    }
    logger().flush();
    thread::sleep(Duration::from_secs(1));
    let files = read_files(TEST_DIR);
    assert_eq!(files.len(), 2);
    let files = read_files(TEST_DIR_OLD);
    if let Some(_max_files) = max_files {
        assert_eq!(files.len(), _max_files);
    } else {
        assert_eq!(files.len(), 5);
    }
}

#[rstest]
#[case(Some(3))]
#[case(Some(2))]
#[case(Some(1))]
#[case(None)]
fn test_buffered_file_rotation_by_size_archive_dir_compress(#[case] max_files: Option<usize>) {
    lock_file!();
    let _ = remove_dir_all(TEST_DIR);
    let rotation =
        Rotation::by_size(1024 * 4 * 2, max_files).archive_dir(TEST_DIR_OLD).compress_exclude(1);
    let base_path = Path::new(TEST_DIR).join("rotation_test.log");
    recipe::buffered_rotated_file_logger(base_path, Level::Debug, rotation)
        .test()
        .build()
        .expect("setup");

    for _i in 0..1000 {
        info!("test {}", _i);
    }
    logger().flush();
    thread::sleep(Duration::from_secs(1));
    let files = read_files(TEST_DIR);
    assert_eq!(files.len(), 2);
    let files = read_files(TEST_DIR_OLD);
    if let Some(_max_files) = max_files {
        assert_eq!(files.len(), _max_files);
        let files_len = files.len();
        if files.len() > 1 {
            let compressed: Vec<DirEntry> = files
                .into_iter()
                .filter(|e| e.file_name().to_string_lossy().ends_with(".gz"))
                .collect();
            assert_eq!(compressed.len() + 1, files_len); // + uncompressed
        }
    } else {
        assert_eq!(files.len(), 5);
    }
}
