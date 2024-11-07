use clap::{ArgAction, Parser};
use common::file_copier;
use common::types::ZpZrOpts;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[clap(version, about = "Promotes files from ZFS snapshots")]

struct Cli {
    /// Print what would happen, without doing it
    #[clap(short, long)]
    noop: bool,
    /// Be verbose
    #[clap(short, long)]
    verbose: bool,
    /// By default, existing live files are overwritten. With this option, they are not
    #[clap(short = 'N', long, action=ArgAction::SetTrue)]
    noclobber: bool,
    /// File(s) to promote
    #[clap(required = true, num_args = 1..)]
    file_list: Vec<String>,
}

fn in_snapshot(file: &Path) -> bool {
    let components: Vec<_> = file.components().map(|s| s.as_os_str()).collect();

    if let Some(zfs_index) = components.iter().position(|&c| c == OsStr::new(".zfs")) {
        if let Some(snapshot_idx) = components.get(zfs_index + 1) {
            return snapshot_idx == &OsStr::new("snapshot");
        }
    }

    false
}

fn target_file(file: &Path) -> Option<PathBuf> {
    let components: Vec<_> = file.components().map(|s| s.as_os_str()).collect();

    if let Some(zfs_index) = components.iter().position(|&c| c == OsStr::new(".zfs")) {
        let ret = components
            .iter()
            .enumerate()
            .filter_map(|(i, c)| {
                if i < zfs_index || i > (zfs_index + 2) {
                    Some(c)
                } else {
                    None
                }
            })
            .collect();
        Some(ret)
    } else {
        None
    }
}

fn main() {
    let cli = Cli::parse();

    let opts = ZpZrOpts {
        verbose: cli.verbose,
        noop: cli.noop,
        noclobber: cli.noclobber,
    };

    let mut errs = 0;

    for file in cli.file_list {
        let file = PathBuf::from(file);

        let file_path = match file.canonicalize() {
            Ok(path) => path,
            Err(e) => {
                eprintln!("Failed to canonicalize {}", e);
                continue;
            }
        };

        if !in_snapshot(&file_path) {
            eprintln!("{} is not inside a ZFS snapshot", &file_path.display());
            errs += 1;
            continue;
        }

        let target_file = match target_file(&file_path) {
            Some(path) => path,
            None => {
                eprintln!("Could not find target for {}", &file_path.display());
                errs += 1;
                continue;
            }
        };

        let target_dir = match target_file.parent() {
            Some(dir) => dir,
            None => {
                eprintln!(
                    "Could not find target directory for {}",
                    &target_file.display()
                );
                errs += 1;
                continue;
            }
        };

        if !target_dir.exists() {
            if opts.verbose {
                println!("Creating {}", target_dir.display());
            }

            if !opts.noop {
                if let Err(e) = fs::create_dir_all(target_dir) {
                    eprintln!("Failed to create directory {}: {}", target_dir.display(), e);
                    errs += 1;
                    continue;
                }
            }
        }

        if let Err(e) = file_copier::copy_file(&file, &target_file, &opts) {
            eprintln!(
                "Failed to copy {} to {}: {}",
                &file.display(),
                &target_file.display(),
                e,
            );
            errs += 1;
        }
    }

    if errs > 0 {
        eprintln!("Encountered {} error(s)", errs);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_target_file() {
        assert_eq!(
            Some(PathBuf::from("/test/dir/file")),
            target_file(&PathBuf::from("/test/.zfs/snapshot/monday/dir/file"))
        );

        assert_eq!(
            Some(PathBuf::from("/test/u01/u02/mtpt/deep/dir/file")),
            target_file(&PathBuf::from(
                "/test/u01/u02/mtpt/.zfs/snapshot/test/deep/dir/file"
            ))
        );
    }

    #[test]
    fn test_in_snapshot() {
        assert!(in_snapshot(&PathBuf::from("/test/.zfs/snapshot/monday/d")));
        assert!(!in_snapshot(&PathBuf::from("/build/dir")));
        assert!(!in_snapshot(&PathBuf::from("/test/snapshot/dir")));
    }
}
