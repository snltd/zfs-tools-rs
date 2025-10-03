use camino::Utf8PathBuf;
use std::env;

pub fn fixture(dir: &str) -> Utf8PathBuf {
    Utf8PathBuf::from_path_buf(env::current_dir().unwrap())
        .unwrap()
        .join("test/resources")
        .join(dir)
}
