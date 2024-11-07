use std::error::Error;
use std::path::PathBuf;

pub type ArgList = Vec<String>;
pub type SnapshotList = Vec<String>;
pub type SnapshotResult = Result<SnapshotList, Box<dyn Error>>;
pub type MountList = Vec<(PathBuf, String)>;
pub type Filesystems = Vec<String>;
pub type ZfsMounts = Vec<(PathBuf, String)>;

pub struct Opts {
    pub verbose: bool,
    pub noop: bool,
}

pub struct ZpZrOpts {
    pub verbose: bool,
    pub noop: bool,
    pub noclobber: bool,
}
