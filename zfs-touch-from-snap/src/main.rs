use clap::Parser;
use common::types::Opts;
use filetime::{set_file_times, FileTime};
use glob::glob;
use std::collections::HashMap;
use std::fs::{metadata, File};
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use time::{format_description::well_known::Rfc2822, Duration, OffsetDateTime};

type MTimeMap = HashMap<PathBuf, SystemTime>;

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
    #[clap()]
    object: Vec<String>,
}

fn touch_directory(dir: &Path, snapshot_name: &str, opts: &Opts) -> Result<(), io::Error> {
    let snapshot_dir = match common::utils::snapshot_dir(dir) {
        Some(snapshot_root) => snapshot_root.join(snapshot_name),
        None => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("{} does not appear to be a ZFS filesystem", dir.display()),
            ))
        }
    };

    if !snapshot_dir.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{} has no ZFS snapshot directory", dir.display()),
        ));
    }

    let live_timestamps = timestamps_for(dir, opts);
    let snapshot_timestamps = timestamps_for(&snapshot_dir, opts);

    let mut errs = 0;

    for (file, ts) in snapshot_timestamps {
        if let Some(live_ts) = live_timestamps.get(&file) {
            let target_file = dir.join(file);
            if &ts != live_ts {
                if opts.noop || opts.verbose {
                    println!("{} -> {}", target_file.display(), format_time(ts));
                }

                if !opts.noop && set_timestamp(&target_file, ts).is_err() {
                    errs += 1;
                }
            }
        }
    }

    if errs == 0 {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Failed to set times in {} files", errs),
        ))
    }
}

fn set_timestamp(file: &Path, ts: SystemTime) -> Result<(), io::Error> {
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

    let mut ret = MTimeMap::new();
    let pattern = format!("{}/**/*", dir.to_string_lossy());

    for entry in glob(&pattern).expect("Failed to read glob pattern") {
        if let Ok(path) = entry {
            if let Ok(metadata) = metadata(&path) {
                if let Ok(relative_path) = path.strip_prefix(dir) {
                    ret.insert(relative_path.to_path_buf(), metadata.modified().unwrap());
                }
            }
        }
    }

    ret
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
