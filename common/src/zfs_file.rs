///! Functions, constants, types, and whatever else comes along, which are required by
///! more than one of the tools in this crate.
///!
use crate::types::{Filesystems, MountList, ZfsMounts};
use crate::zfs_info::dataset_root;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Given a path and a list of ZFS mounts, works out which, if any, filesystem owns the path.
///
pub fn file_to_dataset(file: &Path, mounts: &MountList) -> Option<String> {
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

pub fn files_to_datasets(file_list: &[String], zfs_mounts: ZfsMounts) -> Filesystems {
    let filesystems: HashSet<String> = file_list
        .iter()
        .filter_map(|f| file_to_dataset(&PathBuf::from(f), &zfs_mounts))
        .collect();

    filesystems.into_iter().collect()
}

pub fn snapshot_dir_from_file(file: &Path) -> Option<PathBuf> {
    match dataset_root(file) {
        Ok(dir) => {
            let snapdir = dir.join(".zfs").join("snapshot");
            if snapdir.exists() {
                Some(snapdir)
            } else {
                None
            }
        }
        Err(_) => None,
    }
}

#[cfg(test)]
mod test {
    use super::*;

    // You'll have to trust that these tests pass on my illumos box. They're skipped in Github
    // Actions.
    #[cfg(target_os = "illumos")]
    #[test]
    fn test_snapshot_dir() {
        assert_eq!(
            PathBuf::from("/.zfs/snapshot"),
            snapshot_dir_from_file(&PathBuf::from("/etc/passwd")).unwrap()
        );

        assert_eq!(None, snapshot_dir_from_file(&PathBuf::from("/tmp")));

        assert_eq!(
            PathBuf::from("/build/.zfs/snapshot"),
            snapshot_dir_from_file(&PathBuf::from("/build/omnios-extra/build/")).unwrap()
        );
    }

    #[test]
    fn test_file_to_dataset() {
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
            file_to_dataset(&PathBuf::from("/etc/passwd"), &mounts)
        );

        assert_eq!(
            Some("fast/zone/build/build".to_string()),
            file_to_dataset(&PathBuf::from("/build/file"), &mounts)
        );

        assert_eq!(
            Some("fast/zone/build/config".to_string()),
            file_to_dataset(&PathBuf::from("/build/configs/file"), &mounts)
        );
    }

    #[test]
    fn test_files_to_datasets() {
        let arg_list = &[
            "/build/f1".to_string(),
            "/build/f2".to_string(),
            "/rpool/f3".to_string(),
        ];

        let mount_list = vec![
            (PathBuf::from("/build"), "fast/zone/build/build".to_string()),
            (
                PathBuf::from("/build/configs"),
                "fast/zone/build/config".to_string(),
            ),
            (PathBuf::from("/rpool"), "rpool".to_string()),
        ];

        let mut expected = vec!["fast/zone/build/build".to_string(), "rpool".to_string()];
        let mut actual = files_to_datasets(arg_list, mount_list.clone());
        expected.sort();
        actual.sort();

        assert_eq!(expected, actual);

        assert!(files_to_datasets(&["/where/is/this".to_string()], mount_list).is_empty());
    }
}
