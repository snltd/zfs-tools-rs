use anyhow::ensure;
use clap::Parser;
use common::command_helpers::format_command;
use common::constants::ZFS;
use common::types::{Filesystems, Opts};
use common::{rules, zfs_file, zfs_info};
use std::process::{Command, exit};
use time::{OffsetDateTime, format_description};

#[derive(Parser)]
#[clap(version, about = "Takes automatically named ZFS snapshots", long_about= None)]
struct Cli {
    #[clap(
        short = 't',
        long = "type",
        long_help = "Specify the type of snapshot to take: this  determines the \
        snapshot names\n  e.g  day    @wednesday\n       month  @january\n       \
        date   @2008-30-01\n       time   @08:45\n       now    @2008-30-01_08:45:00"
    )]
    snap_type: String,
    /// Specifies that args are files: the filesystems containing these files will be snapshotted
    #[clap(short, long)]
    files: bool,
    /// Print what would happen, without doing it                                                     
    #[clap(short, long)]
    noop: bool,
    /// Be verbose                                                                                    
    #[clap(short, long)]
    verbose: bool,
    /// Recurse down dataset hierarchies                                                              
    #[clap(short, long)]
    recurse: bool,
    /// Comma-separated list of filesystems to NOT snapshot. Accepts * as a wildcard.
    #[clap(short, long)]
    omit: Option<String>,
    /// Dataset or directory name. If not args are given, every dataset will be snapshotted.
    #[clap()]
    object: Option<Vec<String>>,
}

fn dataset_list(from_user: Option<Vec<String>>, all_filesystems: Filesystems) -> Filesystems {
    match from_user {
        Some(list) => list,
        None => all_filesystems,
    }
}

fn snapname(snap_type: &str, timestamp: OffsetDateTime) -> anyhow::Result<String, String> {
    match snap_type {
        "date" => Ok(timestamp.date().to_string()),
        "day" => Ok(timestamp.weekday().to_string().to_lowercase()),
        "month" => Ok(timestamp.month().to_string().to_lowercase()),
        "time" => format_time(timestamp, "[hour]:[minute]"),
        "now" => format_time(timestamp, "[year]-[month]-[day]_[hour]:[minute]"),
        _ => Err(format!("Unsupported snapshot type: {}", snap_type)),
    }
}

fn format_time(timestamp: OffsetDateTime, format_str: &str) -> anyhow::Result<String, String> {
    let format = format_description::parse(format_str)
        .map_err(|_| "Invalid format description".to_string())?;
    timestamp
        .format(&format)
        .map_err(|_| "Error formatting timestamp".to_string())
}

fn snapshot_exists(snapshot: &str, opts: &Opts) -> bool {
    snapshot_command(snapshot, "list", opts, true)
}

fn destroy_snapshot(snapshot: &str, opts: &Opts) -> bool {
    snapshot_command(snapshot, "destroy", opts, false)
}

fn take_snapshot(snapshot: &str, opts: &Opts) -> bool {
    snapshot_command(snapshot, "snapshot", opts, false)
}

fn snapshot_command(snapshot: &str, action: &str, opts: &Opts, hush: bool) -> bool {
    let mut cmd = Command::new(ZFS);
    cmd.arg(action).arg(snapshot);

    if opts.verbose || opts.noop {
        println!("{}", format_command(&cmd));
    }

    if opts.noop {
        return true;
    }

    let output = cmd
        .output()
        .unwrap_or_else(|_| panic!("failed to run 'zfs {} {}'", action, snapshot));

    if output.status.success() {
        true
    } else {
        if !hush {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!(
                "Error running 'zfs {} {}': {}",
                action,
                snapshot,
                stderr.trim()
            );
        }
        false
    }
}

fn do_the_snapshotting(
    dataset_list: Filesystems,
    snapname: String,
    opts: Opts,
) -> anyhow::Result<()> {
    let mut errs = 0;

    for dataset in dataset_list {
        let snapshot = format!("{}@{}", &dataset, &snapname);
        println!("Snapshotting {}", &snapshot);

        if snapshot_exists(&snapshot, &opts) && !destroy_snapshot(&snapshot, &opts) {
            eprintln!("Failed to destroy existing {}", &snapshot);
            errs += 1;
            continue;
        }

        if !take_snapshot(&snapshot, &opts) {
            eprintln!("Failed to create {}", &snapshot);
            errs += 1;
            continue;
        }
    }

    ensure!(errs == 0, "ERROR: {errs} snapshots were not created");
    Ok(())
}

fn main() {
    let cli = Cli::parse();
    let opts = Opts {
        verbose: cli.verbose,
        noop: cli.noop,
    };

    // If the user gives us a list of files, we don't need this information, and it's potentially
    // expensive.
    let all_filesystems: Vec<String> = if cli.files {
        Vec::new()
    } else {
        zfs_info::all_filesystems().unwrap_or_else(|e| {
            eprintln!("Could not get a list of filesystems: {}", e);
            exit(1);
        })
    };

    let mut dataset_list = if cli.files {
        if cli.object.is_none() {
            eprintln!("-f requires one or more files");
            exit(2);
        }
        match zfs_info::get_mounted_filesystems() {
            Ok(mounts) => zfs_file::files_to_datasets(&cli.object.unwrap(), mounts),
            Err(e) => {
                eprintln!("Failed to get list of mounted filesystems: {}", e);
                exit(1);
            }
        }
    } else if cli.recurse {
        if cli.object.is_none() {
            eprintln!("-r makes no sense without a list of filesystems");
            exit(2);
        } else {
            zfs_info::dataset_list_recursive(cli.object.unwrap(), all_filesystems)
        }
    } else {
        dataset_list(cli.object, all_filesystems)
    };

    if let Some(omit_rules) = cli.omit {
        dataset_list = omit_filesystems(dataset_list, omit_rules);
    }

    if dataset_list.is_empty() {
        println!("Nothing to snapshot.");
        exit(1);
    }

    let now = OffsetDateTime::now_local().expect("Could not get local time");
    let snapname = snapname(&cli.snap_type, now).unwrap_or_else(|_| {
        eprintln!("Invalid snapshot type");
        exit(3);
    });

    match do_the_snapshotting(dataset_list, snapname, opts) {
        Ok(_) => exit(0),
        Err(e) => {
            println!("{}", e);
            exit(4);
        }
    }
}

fn omit_filesystems(filesystem_list: Filesystems, omit_rules: String) -> Filesystems {
    let rules: Vec<_> = omit_rules.split(',').map(|s| s.to_string()).collect();

    filesystem_list
        .into_iter()
        .filter(|item| rules::omit_rules_match(item, &rules))
        .collect()
}

#[cfg(test)]
mod test {
    use super::*;
    use time::{Date, Month, OffsetDateTime, Time, UtcOffset};

    #[test]
    fn test_omit_filesystems() {
        let filesystem_list = vec![
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
            "build/test".to_string(),
            "build/test/a".to_string(),
            "rpool".to_string(),
            "rpool/test_a".to_string(),
        ];

        let mut actual = omit_filesystems(
            filesystem_list.clone(),
            "build,other,rpool/test,other/test".to_string(),
        );

        expected.sort();
        actual.sort();
        assert_eq!(expected, actual);

        expected = vec![
            "rpool".to_string(),
            "rpool/test".to_string(),
            "other".to_string(),
            "other/test".to_string(),
        ];

        actual = omit_filesystems(filesystem_list.clone(), "build*,*a".to_string());

        expected.sort();
        actual.sort();
        assert_eq!(expected, actual);

        expected = vec![
            "build".to_string(),
            "rpool".to_string(),
            "other".to_string(),
        ];

        actual = omit_filesystems(filesystem_list, "*test*".to_string());

        expected.sort();
        actual.sort();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_snapname() {
        let test_time = OffsetDateTime::new_in_offset(
            Date::from_calendar_date(2024, Month::October, 27).expect("date fail"),
            Time::from_hms(9, 45, 23).expect("time fail"),
            UtcOffset::from_hms(0, 0, 0).expect("utc offset fail"),
        );

        assert_eq!("sunday".to_string(), snapname("day", test_time).unwrap());
        assert_eq!("09:45".to_string(), snapname("time", test_time).unwrap());
        assert_eq!("october".to_string(), snapname("month", test_time).unwrap());
        assert_eq!(
            "2024-10-27".to_string(),
            snapname("date", test_time).unwrap()
        );
        assert_eq!(
            "2024-10-27_09:45".to_string(),
            snapname("now", test_time).unwrap()
        );

        assert!(snapname("junk", test_time).is_err());
    }
}
