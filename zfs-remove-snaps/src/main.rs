use clap::Parser;
use common::types::{ArgList, Opts, SnapshotList, SnapshotResult};
use common::utils;
use regex::Regex;
use std::io;
use std::process::{exit, Command};

#[derive(Parser)]
#[clap(version, about = "Bulk-removes ZFS snapshots", long_about = None)]

struct Cli {
    /// Specifies that args are files: the snapshots containing these files will be destroyed
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
    /// Comma-separated list of filesystems from which snapshots should NOT be removed. Accepts * as a wildcard.
    #[clap(short = 'o', long)]
    omit_fs: Option<String>,
    /// Comma-separated list of snapshot names which should NOT be removed. Accepts * as a wildcard.
    #[clap(short = 'O', long)]
    omit_snaps: Option<String>,
    /// Recurse down dataset hierarchies
    #[clap(short, long)]
    recurse: bool,
    /// Dataset, snapshot, or directory name
    #[clap()]
    object: Vec<String>,
}

// If any removal fails, fail the whole lot.
fn remove_snaps(list: SnapshotList, opts: Opts) -> Result<(), std::io::Error> {
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
            println!("{}", utils::format_command(&cmd));
        }

        if !opts.noop {
            cmd.status()?;
        }
    }

    Ok(())
}

fn filter_list(snapshot_list: SnapshotList, omit_rules: &str, is_snapshot: bool) -> SnapshotList {
    let rules: Vec<_> = omit_rules.split(',').map(|s| s.to_string()).collect();

    snapshot_list
        .into_iter()
        .filter(|f| {
            if let Some((fs_name, snap_name)) = f.split_once("@") {
                let item = if is_snapshot { snap_name } else { fs_name };
                utils::omit_rules_match(item, &rules)
            } else {
                false
            }
        })
        .collect()
}

fn filter_by_snap_name(snapshot_list: SnapshotList, omit_rules: &str) -> SnapshotList {
    filter_list(snapshot_list, omit_rules, true)
}

fn filter_by_fs_name(snapshot_list: SnapshotList, omit_rules: &str) -> SnapshotList {
    filter_list(snapshot_list, omit_rules, false)
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

fn snapshot_list(cli: &Cli) -> SnapshotResult {
    let mut arg_list = cli.object.clone();

    if cli.snaps {
        if cli.recurse {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "-r is not allowed with snapshot arguments",
            )));
        } else {
            return snapshot_list_from_snap_names(&arg_list);
        }
    }

    if cli.all {
        if cli.recurse {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "-r is not allowed with dataset name arguments",
            )));
        } else {
            return snapshot_list_from_dataset_names(&arg_list);
        }
    }

    if cli.files {
        let mounts = utils::get_mounted_filesystems()?;
        arg_list = utils::files_to_datasets(&arg_list, mounts);
    }

    if cli.recurse {
        let all_filesystems = utils::all_filesystems()?;
        arg_list = utils::dataset_list_recursive(arg_list, all_filesystems);
    }

    snapshot_list_from_dataset_paths(&arg_list)
}

fn main() {
    let cli = Cli::parse();
    let opts = Opts {
        verbose: cli.verbose,
        noop: cli.noop,
    };

    let mut snapshot_list = match snapshot_list(&cli) {
        Ok(list) => list,
        Err(e) => {
            eprintln!("ERROR: could not generate snapshot list: {}", e);
            exit(1);
        }
    };

    if let Some(omit_snaps) = cli.omit_snaps {
        snapshot_list = filter_by_snap_name(snapshot_list, &omit_snaps);
    }

    if let Some(omit_fs) = cli.omit_fs {
        snapshot_list = filter_by_fs_name(snapshot_list, &omit_fs);
    }

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

    #[test]
    fn test_filter_by_snap_name() {
        let input: SnapshotList = vec![
            "rpool/test@snap1".to_string(),
            "rpool/test@snap2".to_string(),
            "rpool/test@mysnap1".to_string(),
            "rpool/test@other".to_string(),
        ];

        let expected1: SnapshotList = vec!["rpool/test@mysnap1".to_string()];

        assert_eq!(expected1, filter_by_snap_name(input.clone(), "snap*,other"));

        let expected2: SnapshotList = vec![
            "rpool/test@snap2".to_string(),
            "rpool/test@other".to_string(),
        ];

        assert_eq!(expected2, filter_by_snap_name(input.clone(), "*1"));

        let expected3: SnapshotList = vec![
            "rpool/test@snap1".to_string(),
            "rpool/test@snap2".to_string(),
            "rpool/test@mysnap1".to_string(),
        ];

        assert_eq!(expected3, filter_by_snap_name(input.clone(), "*t*"));

        assert_eq!(
            input,
            filter_by_snap_name(input.clone(), "nothing,matches,*this")
        );
    }

    #[test]
    fn test_filter_by_fs_name() {
        let input: SnapshotList = vec![
            "rpool/test1@snap1".to_string(),
            "rpool/test2@snap2".to_string(),
            "rpool/test1@mysnap1".to_string(),
            "test/data@snap".to_string(),
            "rpool/test@other".to_string(),
        ];

        let expected1: SnapshotList = vec![
            "rpool/test1@snap1".to_string(),
            "rpool/test2@snap2".to_string(),
            "rpool/test1@mysnap1".to_string(),
            "rpool/test@other".to_string(),
        ];

        assert_eq!(expected1, filter_by_fs_name(input.clone(), "test/*"));

        let expected2: SnapshotList = vec![
            "rpool/test2@snap2".to_string(),
            "test/data@snap".to_string(),
            "rpool/test@other".to_string(),
        ];

        assert_eq!(expected2, filter_by_fs_name(input.clone(), "*1"));

        let expected3: SnapshotList = vec![
            "rpool/test2@snap2".to_string(),
            "test/data@snap".to_string(),
            "rpool/test@other".to_string(),
        ];

        assert_eq!(expected3, filter_by_fs_name(input.clone(), "*test1,test2"));

        let expected4: SnapshotList = vec![];
        assert_eq!(expected4, filter_by_fs_name(input.clone(), "*t*"));

        assert_eq!(input, filter_by_fs_name(input.clone(), "snap"));
    }
}
