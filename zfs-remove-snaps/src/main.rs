use clap::Parser;
use common::utils;
use regex::Regex;
use std::collections::HashSet;
use std::error::Error;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{exit, Command};

type ArgList = Vec<String>;
type SnapshotList = Vec<String>;
type SnapshotResult = Result<SnapshotList, Box<dyn Error>>;
type MountList = Vec<(PathBuf, String)>;

struct CommonOpts {
    verbose: bool,
    noop: bool,
}

#[derive(Parser)]
#[clap(version, about = "Bulk-removes ZFS snapshots", long_about = None)]

struct Cli {
    /// Specifies that args are files: the snapshots containing these fileswill be destroyed
    #[clap(short, long)]
    files: bool,
    /// purge ALL datasets with this name ANYWHERE in the hierarchy
    #[clap(short = 'a', long = "all-datasets")]
    all: bool,
    /// Specifies that all args are snapshot names
    #[clap(short = 's', long = "snaps")]
    snaps: bool,
    /// Print what would happen, without doing it
    #[clap(short, long)]
    noop: bool,
    /// Be verbose
    #[clap(short, long)]
    verbose: bool,
    /// Recurse down dataset hierarchies
    #[clap(short, long)]
    recurse: bool,
    /// Dataset, snapshot, or directory name
    #[clap()]
    object: Vec<String>,
}

// Returns a vec of ZFS mounts, sorted by the length of the path
fn zfs_mounts(mounts: Vec<String>) -> Result<MountList, Box<dyn Error>> {
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

fn dataset_from_file(file: &Path, mounts: &MountList) -> Option<String> {
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

fn snapshot_list_from_file_names(list: &ArgList, mounts: MountList) -> SnapshotResult {
    let datasets: HashSet<String> = list
        .iter()
        .filter_map(|f| dataset_from_file(&PathBuf::from(f), &mounts))
        .collect();

    snapshot_list_from_dataset_paths(&datasets.into_iter().collect())
}

// Not to be confused with snapshot_list_from_dataset_names(), which only expects
// the last segment of the name. This uses the whole path.
fn snapshot_list_from_dataset_paths(dataset_list: &ArgList) -> SnapshotResult {
    let ret: SnapshotList = utils::all_snapshots()?
        .iter()
        .filter_map(|line| {
            if dataset_list
                .iter()
                .any(|dataset| line.starts_with(&format!("{}@", dataset)))
            {
                Some(line.to_string())
            } else {
                None
            }
        })
        .collect();

    Ok(ret)
}

// All snapshots whose dataset name (final part) is one of those given.
fn snapshot_list_from_dataset_names(dataset_list: &ArgList) -> SnapshotResult {
    let patterns: Result<Vec<Regex>, _> = dataset_list
        .iter()
        .map(|dataset| Regex::new(&format!(r"/{}@", regex::escape(dataset))))
        .collect();

    let patterns = patterns?;

    let ret: SnapshotList = utils::all_snapshots()?
        .iter()
        .filter_map(|line| {
            if patterns.iter().any(|pattern| pattern.is_match(line)) {
                Some(line.to_string())
            } else {
                None
            }
        })
        .collect();

    Ok(ret)
}

// All snapshots with the names given in list
fn snapshot_list_from_snap_names(snaplist: &ArgList) -> SnapshotResult {
    let ret = utils::all_snapshots()?
        .iter()
        .filter_map(|line| {
            if snaplist
                .iter()
                .any(|snap| line.ends_with(&format!("@{}", snap)))
            {
                Some(line.to_string())
            } else {
                None
            }
        })
        .collect();

    Ok(ret)
}

fn format_command(cmd: &Command) -> String {
    format!(
        "{} {}",
        cmd.get_program().to_string_lossy(),
        cmd.get_args()
            .map(|arg| arg.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ")
    )
}

// If any removal fails, fail the whole lot.
fn remove_snaps(list: SnapshotList, opts: CommonOpts) -> Result<(), std::io::Error> {
    for snap in list {
        // Double check that we aren't going to remove a dataset
        if !snap.contains("@") {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("refusing to remove {}", snap),
            ));
        }

        let mut cmd = Command::new(utils::ZFS);
        cmd.arg("destroy").arg(&snap);

        if opts.verbose || opts.noop {
            println!("{}", format_command(&cmd));
        }

        if !opts.noop {
            cmd.status()?;
        }
    }

    Ok(())
}

fn snapshot_list(cli: &Cli) -> SnapshotResult {
    if cli.snaps {
        snapshot_list_from_snap_names(&cli.object)
    } else if cli.all {
        snapshot_list_from_dataset_names(&cli.object)
    } else if cli.files {
        let all_mounts = utils::all_zfs_mounts()?;
        let zfs_mounts = zfs_mounts(all_mounts)?;
        snapshot_list_from_file_names(&cli.object, zfs_mounts)
    } else {
        snapshot_list_from_dataset_paths(&cli.object)
    }
}

fn main() {
    let cli = Cli::parse();
    let opts = CommonOpts {
        verbose: cli.verbose,
        noop: cli.noop,
    };

    let snapshot_list = match snapshot_list(&cli) {
        Ok(list) => list,
        Err(e) => {
            eprintln!("ERROR: could not generate snapshot list: {}", e);
            exit(1);
        }
    };

    if snapshot_list.is_empty() {
        println!("No snapshots to remove.");
        exit(0);
    }

    if let Err(err) = remove_snaps(snapshot_list, opts) {
        eprintln!("ERROR: could not remove snapshot: {}", err);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::fs::read_to_string;

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
