use anyhow::anyhow;
use clap::Parser;
use common::types::Opts;
use common::zfs_file;
use common::zfs_info::dataset_root;
use filetime::{set_file_times, FileTime};
use glob::glob;
use std::collections::BTreeMap;
use std::fs::{metadata, File};
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use time::{format_description::well_known::Rfc2822, Duration, OffsetDateTime};

type MTimeMap = BTreeMap<PathBuf, SystemTime>;

#[derive(Parser)]
#[clap(version, about = "Aligns file timestamps with those in a given snapshot", long_about = None)]
struct Cli {
    /// use specified snapshot name, rather than yesterday's
    #[clap(short, long)]
    snapname: Option<String>,
    /// Print what would happen, without doing it
    #[clap(short, long)]
    noop: bool,
    /// Be verbose
    #[clap(short, long)]
    verbose: bool,
    /// directory name
    #[arg(required = true)]
    object: Vec<String>,
}

fn touch_directory(dir: &Path, snapshot_name: &str, opts: &Opts) -> anyhow::Result<()> {
    let snapshot_top_level = match zfs_file::snapshot_dir_from_file(dir) {
        Some(snapshot_root) => snapshot_root.join(snapshot_name),
        None => {
            return Err(anyhow!(
                "{} does not appear to be a ZFS filesystem",
                dir.display()
            ))
        }
    };

    if !snapshot_top_level.exists() {
        return Err(anyhow!("{} has no ZFS snapshot directory", dir.display()));
    }

    let dataset_root = dataset_root(dir)?;
    let snapshot_dir: PathBuf;

    if dir == dataset_root {
        snapshot_dir = snapshot_top_level;
    } else {
        let relative_path = dir
            .to_string_lossy()
            .replace(&format!("{}/", dataset_root.to_string_lossy()), "");

        snapshot_dir = snapshot_top_level.join(&relative_path);
        println!("relative_path is {:?}", &relative_path);
    }

    // println!("snapshot_top_level is {:?}", snapshot_top_level);
    // println!("dataset_root is {:?}", dataset_root);
    // println!("dir is {:?}", dir);

    if !snapshot_dir.exists() {
        return Err(anyhow!("No source directory: {}", snapshot_dir.display()));
    }

    let live_timestamps = timestamps_for(dir, opts);
    let snapshot_timestamps = timestamps_for(&snapshot_dir, opts);

    println!(
        "{} in live and {} in snapshot",
        live_timestamps.len(),
        snapshot_timestamps.len()
    );

    let mut errs = 0;

    for (file, ts) in snapshot_timestamps {
        if let Some(live_ts) = live_timestamps.get(&file) {
            let target_file = dir.join(&file);
            if &ts != live_ts {
                if opts.noop || opts.verbose {
                    println!("{} -> {}", target_file.display(), format_time(ts));
                }

                if !opts.noop && set_timestamp(&target_file, ts).is_err() {
                    errs += 1;
                }
            } else if opts.verbose {
                println!("{} : correct", file.display());
            }
        } else if opts.verbose {
            println!("{} : no source in snapshot", file.display());
        }
    }

    if errs == 0 {
        Ok(())
    } else {
        Err(anyhow!("Failed to set times in {} files", errs))
    }
}

fn set_timestamp(file: &Path, ts: SystemTime) -> io::Result<()> {
    println!("setting {}", file.display());
    let mtime = FileTime::from_system_time(ts);
    File::open(file)?;
    set_file_times(file, mtime, mtime)
}

fn format_time(time: SystemTime) -> String {
    let datetime = OffsetDateTime::from(time);
    datetime.format(&Rfc2822).unwrap()
}

fn timestamps_for(dir: &Path, opts: &Opts) -> MTimeMap {
    if opts.verbose {
        println!("Collecting timestamps for {}", dir.display());
    }

    let pattern = format!("{}/**/*", dir.to_string_lossy());
    glob(&pattern)
        .expect("Failed to read glob pattern")
        .filter_map(Result::ok)
        .filter_map(|path| {
            let metadata = metadata(&path).ok()?;
            let relative_path = path.strip_prefix(dir).ok()?;
            let modified_time = metadata.modified().ok()?;
            Some((relative_path.to_path_buf(), modified_time))
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

    for f in cli.object {
        let f = PathBuf::from(f);
        if !f.is_dir() {
            println!("WARNING: {} is not a valid directory", f.display());
            continue;
        }

        if let Err(e) = touch_directory(&f, &snapname, &opts) {
            eprintln!("ERROR: {}", e);
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

        let result = timestamps_for(&PathBuf::from("test/resources"), &opts);
        let mut actual_files: Vec<PathBuf> = result.keys().cloned().collect();

        let mut expected_files = vec![
            PathBuf::from("dir1"),
            PathBuf::from("dir1/file3"),
            PathBuf::from("dir2"),
            PathBuf::from("dir2/dir3"),
            PathBuf::from("dir2/dir3/file5"),
            PathBuf::from("dir2/file4"),
            PathBuf::from("file1"),
            PathBuf::from("file2"),
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
