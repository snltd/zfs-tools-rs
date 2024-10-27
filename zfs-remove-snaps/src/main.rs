use clap::Parser;
use common::types::{ArgList, MountList, Opts, SnapshotList, SnapshotResult};
use common::utils;
use regex::Regex;
use std::collections::HashSet;
use std::io;
use std::path::PathBuf;
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
    /// Recurse down dataset hierarchies
    #[clap(short, long)]
    recurse: bool,
    /// Dataset, snapshot, or directory name
    #[clap()]
    object: Vec<String>,
}

fn snapshot_list_from_file_names(list: &ArgList, mounts: MountList) -> SnapshotResult {
    let datasets: HashSet<String> = list
        .iter()
        .filter_map(|f| utils::dataset_from_file(&PathBuf::from(f), &mounts))
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

fn snapshot_list(cli: &Cli) -> SnapshotResult {
    if cli.snaps {
        snapshot_list_from_snap_names(&cli.object)
    } else if cli.all {
        snapshot_list_from_dataset_names(&cli.object)
    } else if cli.files {
        let all_mounts = utils::all_zfs_mounts()?;
        let zfs_mounts = utils::zfs_mounts(all_mounts)?;
        snapshot_list_from_file_names(&cli.object, zfs_mounts)
    } else {
        snapshot_list_from_dataset_paths(&cli.object)
    }
}

fn main() {
    let cli = Cli::parse();
    let opts = Opts {
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
