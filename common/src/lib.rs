//! Functions, constants, and whatever else comes along, which are required by
//! more than one of the tools in this crate.
//!
pub mod utils {
    use crate::types::MountList;
    use std::error::Error;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    pub const ZFS: &str = "/usr/sbin/zfs";

    /// Returns a Vec of all the snapshots zfs can see, each being a string.
    ///
    pub fn all_snapshots() -> Result<Vec<String>, Box<dyn Error>> {
        let mut cmd = Command::new(ZFS);
        cmd.arg("list")
            .arg("-Ho")
            .arg("name")
            .arg("-t")
            .arg("snapshot");

        output_as_lines(cmd)
    }

    /// Returns a Vec of all the ZFS filesystems on the host, each being a string.
    ///
    pub fn all_filesystems() -> Result<Vec<String>, Box<dyn Error>> {
        let mut cmd = Command::new(ZFS);
        cmd.arg("list")
            .arg("-Ho")
            .arg("name")
            .arg("-t")
            .arg("filesystem");

        output_as_lines(cmd)
    }

    /// Returns a Vec of all mounted ZFS filesystems, described as Strings.
    ///
    pub fn all_zfs_mounts() -> Result<Vec<String>, Box<dyn Error>> {
        let mut cmd = Command::new(ZFS);
        cmd.arg("list").arg("-Ho").arg("mountpoint,name");
        output_as_lines(cmd)
    }

    /// Takes a Command output and returns it as a Vec of strings. Empty lines
    /// are omitted.
    ///
    pub fn output_as_lines(mut cmd: Command) -> Result<Vec<String>, Box<dyn Error>> {
        let raw_output = cmd.output()?;
        let string_output = String::from_utf8(raw_output.stdout)?;
        let lines: Vec<String> = string_output.lines().map(String::from).collect();

        Ok(lines)
    }

    /// Given a path and a list of ZFS mounts, works out which, if any, ZFS
    /// filesystem owns the path.
    ///
    pub fn dataset_from_file(file: &Path, mounts: &MountList) -> Option<String> {
        file.ancestors().find_map(|f| {
            mounts.iter().find_map(|(mountpoint, name)| {
                if f.starts_with(mountpoint) {
                    Some(name.clone())
                } else {
                    None
                }
            })
        })
    }

    /// Returns a vec of all the ZFS mounts which are not 'legacy', sorted by the
    /// length of the path
    ///
    pub fn zfs_mounts(mounts: Vec<String>) -> Result<MountList, Box<dyn Error>> {
        let mut ret: Vec<(PathBuf, String)> = mounts
            .iter()
            .filter_map(|line| {
                let mut parts = line.split_whitespace();
                match (parts.next(), parts.next()) {
                    (Some(mountpoint), Some(name))
                        if mountpoint != "none" && mountpoint != "legacy" =>
                    {
                        Some((PathBuf::from(mountpoint), name.to_string()))
                    }
                    _ => None,
                }
            })
            .collect();

        ret.sort_by_key(|(path, _name)| std::cmp::Reverse(path.to_string_lossy().len()));
        Ok(ret)
    }

    /// Returns a printable string of the given command
    ///
    pub fn format_command(cmd: &Command) -> String {
        format!(
            "{} {}",
            cmd.get_program().to_string_lossy(),
            cmd.get_args()
                .map(|arg| arg.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" ")
        )
    }
}

#[cfg(test)]
mod test {
    use super::utils::*;
    use std::fs::read_to_string;
    use std::path::PathBuf;
    use std::process::Command;

    #[test]
    fn test_output_as_lines() {
        assert_eq!(
            Vec::<String>::new(),
            output_as_lines(Command::new("/bin/true")).unwrap()
        );

        let expected: Vec<String> = vec![
            "Cargo.toml".to_string(),
            "src".to_string(),
            "test".to_string(),
        ];

        assert_eq!(expected, output_as_lines(Command::new("/bin/ls")).unwrap());
    }

    #[test]
    fn test_zfs_mounts() {
        let expected: Vec<(PathBuf, String)> = vec![
            (
                PathBuf::from("/zones/serv-build"),
                "rpool/zones/serv-build".to_string(),
            ),
            (
                PathBuf::from("/build/configs"),
                "fast/zone/build/config".to_string(),
            ),
            (PathBuf::from("/build"), "fast/zone/build/build".to_string()),
            (PathBuf::from("/rpool"), "rpool".to_string()),
            (PathBuf::from("/zones"), "rpool/zones".to_string()),
        ];

        assert_eq!(
            expected,
            zfs_mounts(
                read_to_string("test/resources/mountpoint_list.txt")
                    .unwrap()
                    .lines()
                    .map(String::from)
                    .collect()
            )
            .unwrap()
        );
    }

    #[test]
    fn test_dataset_from_file() {
        let mounts: Vec<(PathBuf, String)> = vec![
            (
                PathBuf::from("/zones/serv-build"),
                "rpool/zones/serv-build".to_string(),
            ),
            (
                PathBuf::from("/build/configs"),
                "fast/zone/build/config".to_string(),
            ),
            (PathBuf::from("/build"), "fast/zone/build/build".to_string()),
            (PathBuf::from("/rpool"), "rpool".to_string()),
            (PathBuf::from("/zones"), "rpool/zones".to_string()),
        ];

        assert_eq!(
            None,
            dataset_from_file(&PathBuf::from("/etc/passwd"), &mounts)
        );

        assert_eq!(
            Some("fast/zone/build/build".to_string()),
            dataset_from_file(&PathBuf::from("/build/file"), &mounts)
        );

        assert_eq!(
            Some("fast/zone/build/config".to_string()),
            dataset_from_file(&PathBuf::from("/build/configs/file"), &mounts)
        );
    }
}

pub mod types {
    use std::error::Error;
    use std::path::PathBuf;

    pub type ArgList = Vec<String>;
    pub type SnapshotList = Vec<String>;
    pub type SnapshotResult = Result<SnapshotList, Box<dyn Error>>;
    pub type MountList = Vec<(PathBuf, String)>;

    pub struct Opts {
        pub verbose: bool,
        pub noop: bool,
    }
}
