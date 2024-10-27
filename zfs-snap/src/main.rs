use clap::Parser;
use common::types::Opts;
use common::utils;
use std::collections::HashSet;
use std::path::PathBuf;
use std::process::{exit, Command};
use time::{format_description, OffsetDateTime};

type Filesystems = Vec<String>;
type ZfsMounts = Vec<(PathBuf, String)>;

#[derive(Parser)]
#[clap(version, about = "Takes automatically named ZFS snapshots", long_about= None)]

struct Cli {
    #[clap(
        short = 't',
        long,
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
    /// Comma-separated list of filesystems to NOT snapshot. Accepts Rust regexes.
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

fn dataset_list_recursive(from_user: Vec<String>, all_filesystems: Filesystems) -> Filesystems {
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

fn files_to_datasets(file_list: Vec<String>, zfs_mounts: ZfsMounts) -> Filesystems {
    let filesystems: HashSet<String> = file_list
        .iter()
        .filter_map(|f| utils::dataset_from_file(&PathBuf::from(f), &zfs_mounts))
        .collect();

    filesystems.into_iter().collect()
}

fn snapname(snap_type: &str, timestamp: OffsetDateTime) -> Result<String, String> {
    match snap_type {
        "date" => Ok(timestamp.date().to_string()),
        "day" => Ok(timestamp.weekday().to_string().to_lowercase()),
        "month" => Ok(timestamp.month().to_string().to_lowercase()),
        "time" => format_time(timestamp, "[hour]:[minute]"),
        "now" => format_time(timestamp, "[year]-[month]-[day]_[hour]:[minute]"),
        _ => Err(format!("Unsupported snapshot type: {}", snap_type)),
    }
}

fn format_time(timestamp: OffsetDateTime, format_str: &str) -> Result<String, String> {
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
    let mut cmd = Command::new(utils::ZFS);
    cmd.arg(action).arg(snapshot);

    if opts.verbose || opts.noop {
        println!("{}", utils::format_command(&cmd));
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
) -> Result<(), std::io::Error> {
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

    if errs > 0 {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("ERROR: {} snapshots were not created", errs),
        ))
    } else {
        Ok(())
    }
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
        utils::all_filesystems().unwrap_or_else(|e| {
            eprintln!("Could not get a list of filesystems: {}", e);
            exit(1);
        })
    };

    let mut dataset_list = if cli.files {
        if cli.object.is_none() {
            eprintln!("-f requires one or more files");
            exit(2);
        }

        let all_mounts = utils::all_zfs_mounts().expect("could not get ZFS mounts");
        let zfs_mounts = utils::zfs_mounts(all_mounts).expect("could not process ZFS mounts");
        files_to_datasets(cli.object.unwrap(), zfs_mounts)
    } else if cli.recurse {
        if cli.object.is_none() {
            eprintln!("-r makes no sense without a list of filesystems");
            exit(2);
        } else {
            dataset_list_recursive(cli.object.unwrap(), all_filesystems)
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
        .filter(|f| {
            !rules.iter().any(|rule| match rule.as_str() {
                r if r.starts_with('*') && r.ends_with('*') => f.contains(&r[1..r.len() - 1]),
                r if r.starts_with('*') => f.ends_with(&r[1..]),
                r if r.ends_with('*') => f.starts_with(&r[..r.len() - 1]),
                r => f == r,
            })
        })
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

    #[test]
    fn test_files_to_datasets() {
        let arg_list = vec![
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

        assert!(files_to_datasets(vec!["/where/is/this".to_string()], mount_list).is_empty());
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
