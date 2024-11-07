use crate::command_helpers::output_as_lines;
use crate::constants::ZFS;
use crate::types::{Filesystems, MountList};
use std::collections::HashSet;
use std::error::Error;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{fs, io};

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

/// Returns a vec of all the ZFS mounts which are not 'legacy', sorted by the
/// length of the path
///
pub fn mounted_filesystems(mounts: Vec<String>) -> Result<MountList, Box<dyn Error>> {
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

pub fn get_mounted_filesystems() -> Result<MountList, Box<dyn Error>> {
    let all_mounts = all_zfs_mounts()?;
    mounted_filesystems(all_mounts)
}

pub fn is_mountpoint(file: &Path) -> io::Result<bool> {
    if file == PathBuf::from("/") {
        Ok(true)
    } else {
        let path_metadata = fs::metadata(file)?;
        let parent_metadata = fs::metadata(file.parent().unwrap_or(file))?;
        Ok(path_metadata.dev() != parent_metadata.dev())
    }
}

pub fn dataset_root(file: &Path) -> io::Result<PathBuf> {
    if is_mountpoint(file)? {
        Ok(file.to_path_buf())
    } else if let Some(parent) = file.parent() {
        dataset_root(parent)
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "failed to find root",
        ))
    }
}

/// Given a list of ZFS filesystems and knowledge of all ZFS filesystems, returns the subset
/// of all filesystems under any of the given ones.
///
pub fn dataset_list_recursive(from_user: Vec<String>, all_filesystems: Filesystems) -> Filesystems {
    let unique_datasets: HashSet<String> = from_user
        .into_iter()
        .flat_map(|path| {
            let formatted_path = ensure_trailing_slash(&path);
            all_filesystems
                .iter()
                .filter(move |fs| *fs == &path || fs.starts_with(&formatted_path))
                .map(|fs| fs.to_owned())
        })
        .collect();

    unique_datasets.into_iter().collect()
}

fn ensure_trailing_slash(path: &str) -> String {
    if path.ends_with('/') {
        path.to_string()
    } else {
        format!("{}/", path)
    }
}

#[cfg(test)]
mod test {
    use super::*;

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
            mounted_filesystems(
                fs::read_to_string("test/resources/mountpoint_list.txt")
                    .unwrap()
                    .lines()
                    .map(String::from)
                    .collect()
            )
            .unwrap()
        );
    }

    #[test]
    fn test_dataset_list_recursive() {
        let arg_list = vec!["build".to_string(), "rpool/test".to_string()];

        let all_filesystems = vec![
            "build".to_string(),
            "build/test".to_string(),
            "build/test/a".to_string(),
            "rpool".to_string(),
            "rpool/test".to_string(),
            "rpool/test_a".to_string(),
            "other".to_string(),
            "other/test".to_string(),
        ];

        let mut expected = vec![
            "build".to_string(),
            "build/test".to_string(),
            "build/test/a".to_string(),
            "rpool/test".to_string(),
        ];

        let mut actual = dataset_list_recursive(arg_list, all_filesystems);

        expected.sort();
        actual.sort();

        assert_eq!(expected, actual);
    }
}
