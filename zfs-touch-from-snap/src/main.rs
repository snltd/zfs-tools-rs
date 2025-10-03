use anyhow::{bail, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use common::types::Opts;
use common::verbose;
use common::zfs_file;
use common::zfs_info::dataset_root;
use filetime::{set_file_times, FileTime};
use glob::glob;
use std::collections::BTreeMap;
use std::fs::{metadata, File};
use std::io;
use std::time::SystemTime;
use time::{format_description::well_known::Rfc2822, Duration, OffsetDateTime};

type MTimeMap = BTreeMap<Utf8PathBuf, SystemTime>;

#[derive(Parser)]
#[clap(version, about = "Aligns file timestamps with those in a given snapshot", long_about = None)]
struct Cli {
    /// Use specified snapshot name, rather than yesterday's
    #[clap(short, long)]
    snapname: Option<String>,
    /// Print what would happen, without doing it
    #[clap(short, long)]
    noop: bool,
    /// Be verbose
    #[clap(short, long)]
    verbose: bool,
    /// One or more directories
    #[arg(required = true)]
    dirs: Vec<String>,
}

fn touch_directory(dir: &Utf8Path, snapshot_name: &str, opts: &Opts) -> anyhow::Result<()> {
    verbose!(opts, "Touching directory {}", dir);

    let snapshot_top_level = match zfs_file::snapshot_dir_from_file(dir) {
        Some(snapshot_root) => snapshot_root.join(snapshot_name),
        None => bail!("{} does not appear to be a ZFS filesystem", dir),
    };

    ensure!(
        snapshot_top_level.exists(),
        "No readable ZFS snapshot directory. (Expected '{}')",
        snapshot_top_level
    );

    let dataset_root = dataset_root(dir)?;

    let snapshot_dir = if dir == dataset_root {
        snapshot_top_level
    } else {
        let relative_path = dir.to_string().replace(&format!("{dataset_root}/"), "");
        snapshot_top_level.join(&relative_path)
    };

    ensure!(
        snapshot_dir.exists(),
        "No source directory: {}",
        snapshot_dir
    );

    let live_timestamps = timestamps_for(dir, opts);
    let snapshot_timestamps = timestamps_for(&snapshot_dir, opts);
    let mut errs = 0;

    for (file, ts) in snapshot_timestamps {
        if let Some(live_ts) = live_timestamps.get(&file) {
            let target_file = dir.join(&file);
            if &ts != live_ts {
                verbose!(opts, "{target_file} -> {}", format_time(ts));

                if !opts.noop && set_timestamp(&target_file, ts).is_err() {
                    errs += 1;
                }
            } else {
                verbose!(opts, "{file} : correct");
            }
        } else {
            verbose!(opts, "{file} : no source in snapshot");
        }
    }

    ensure!(errs == 0, "Failed to set times in {} files", errs);

    Ok(())
}

fn set_timestamp(file: &Utf8Path, ts: SystemTime) -> io::Result<()> {
    let mtime = FileTime::from_system_time(ts);
    File::open(file)?;
    set_file_times(file, mtime, mtime)
}

fn format_time(time: SystemTime) -> String {
    let datetime = OffsetDateTime::from(time);
    datetime.format(&Rfc2822).unwrap()
}

fn timestamps_for(dir: &Utf8Path, opts: &Opts) -> MTimeMap {
    verbose!(opts, "Collecting timestamps for {}", dir);

    let pattern = format!("{}/**/*", dir);

    glob(&pattern)
        .expect("Failed to read glob pattern")
        .filter_map(Result::ok)
        .filter_map(|path| {
            let metadata = metadata(&path).ok()?;
            let relative_path = path.strip_prefix(dir).ok()?;
            let modified_time = metadata.modified().ok()?;
            let utf8_path = Utf8PathBuf::from_path_buf(relative_path.to_path_buf()).ok()?;
            Some((utf8_path, modified_time))
        })
        .collect()
}

fn default_snapname(ts: OffsetDateTime) -> String {
    let yesterday = ts - Duration::days(1);
    yesterday.weekday().to_string().to_lowercase()
}

fn main() {
    let cli = Cli::parse();

    let opts = Opts {
        verbose: cli.verbose,
        noop: cli.noop,
    };

    let snapname = match cli.snapname {
        Some(name) => name,
        None => {
            let today = OffsetDateTime::now_local().expect("Failed to get local time");
            default_snapname(today)
        }
    };

    for f in cli.dirs {
        let path = Utf8PathBuf::from(f);

        let full_path = match path.canonicalize_utf8() {
            Ok(f) => f,
            Err(_) => {
                eprintln!("ERROR: cannot cannonicalize {path}");
                std::process::exit(1);
            }
        };

        if !full_path.is_dir() {
            eprintln!("WARNING: {full_path} is not a valid directory");
            continue;
        }

        if let Err(e) = touch_directory(&full_path, &snapname, &opts) {
            eprintln!("ERROR: {e}");
            std::process::exit(1)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use time::{Date, Month, OffsetDateTime, Time, UtcOffset};

    #[test]
    fn test_timestamps_for() {
        let opts = Opts {
            verbose: false,
            noop: false,
        };

        let result = timestamps_for(&Utf8PathBuf::from("test/resources"), &opts);
        let mut actual_files: Vec<Utf8PathBuf> = result.keys().cloned().collect();

        let mut expected_files = vec![
            Utf8PathBuf::from("dir1"),
            Utf8PathBuf::from("dir1/file3"),
            Utf8PathBuf::from("dir2"),
            Utf8PathBuf::from("dir2/dir3"),
            Utf8PathBuf::from("dir2/dir3/file5"),
            Utf8PathBuf::from("dir2/file4"),
            Utf8PathBuf::from("file1"),
            Utf8PathBuf::from("file2"),
        ];

        expected_files.sort();
        actual_files.sort();

        assert_eq!(expected_files, actual_files);
    }

    #[test]
    fn test_default_snapname() {
        let test_time = OffsetDateTime::new_in_offset(
            Date::from_calendar_date(2024, Month::October, 28).expect("date fail"),
            Time::from_hms(9, 45, 23).expect("time fail"),
            UtcOffset::from_hms(0, 0, 0).expect("utc offset fail"),
        );

        assert_eq!("sunday".to_string(), default_snapname(test_time));
    }
}
