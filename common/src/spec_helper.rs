use std::env::current_dir;
use std::path::PathBuf;

pub fn fixture(dir: &str) -> PathBuf {
    current_dir().unwrap().join("test/resources").join(dir)
}
