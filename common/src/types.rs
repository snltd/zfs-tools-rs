use camino::Utf8PathBuf;

pub type ArgList = Vec<String>;
pub type SnapshotList = Vec<String>;
pub type SnapshotResult = anyhow::Result<SnapshotList>;
pub type MountList = Vec<(Utf8PathBuf, String)>;
pub type Filesystems = Vec<String>;
pub type ZfsMounts = Vec<(Utf8PathBuf, String)>;

pub struct Opts {
    pub verbose: bool,
    pub noop: bool,
}

pub struct ZpZrOpts {
    pub verbose: bool,
    pub noop: bool,
    pub noclobber: bool,
}
