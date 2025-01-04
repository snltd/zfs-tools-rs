mod types;
mod user_interaction;

use crate::types::{Candidate, Candidates, CopyAction};
use clap::{ArgAction, Parser};
use common::constants::DIFF;
use common::types::ZpZrOpts;
use common::{file_copier, zfs_info};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{fs, io};

#[derive(Parser)]
#[clap(version, about = "Restores files from ZFS snapshots")]
struct Cli {
    /// Print what would happen, without doing it
    #[clap(short, long)]
    noop: bool,
    /// Be verbose
    #[clap(short, long)]
    verbose: bool,
    /// Automatically recover the newest backup
    #[clap(short, long)]
    auto: bool,
    /// By default, existing live files are overwritten. With this option, they are not
    #[clap(short = 'N', long, action=ArgAction::SetTrue)]
    noclobber: bool,
    /// File(s) to restore
    #[clap(required = true, num_args = 1..)]
    file_list: Vec<String>,
}

fn all_snapshot_dirs(dataset_root: &Path) -> Option<Vec<PathBuf>> {
    let snapshot_root = dataset_root.join(".zfs").join("snapshot");

    if snapshot_root.exists() {
        std::fs::read_dir(snapshot_root)
            .ok()?
            .map(|entry| entry.ok().map(|f| f.path()))
            .collect()
    } else {
        None
    }
}

fn restore_action(file: &Path, cli: &Cli) -> anyhow::Result<CopyAction> {
    // file may well not exist, so let's assume user error if its PARENT isn't there
    let parent = file.parent().unwrap();
    let target_dir = parent.canonicalize()?;
    let filesystem_root = zfs_info::dataset_root(&target_dir)?;
    let mut candidates = candidates(&filesystem_root, file, cli)?;

    if candidates.is_empty() {
        println!("No matches found.");
        return Ok(None);
    }

    candidates.sort_by_key(|c| std::cmp::Reverse(c.mtime));

    let original_file = original_details(file)?;

    let choice_tuple = if cli.auto {
        Some((0_usize, None))
    } else {
        user_interaction::print_options(original_file, &candidates);
        let user_input = user_interaction::get_choice()?;
        user_interaction::parse_choice(&user_input)
    };

    if choice_tuple.is_none() {
        return Ok(None);
    }

    let (candidate_index, command_option) = choice_tuple.unwrap();

    let candidate_object = match candidates.get(candidate_index) {
        Some(obj) => obj,
        None => {
            eprintln!("Cannot find requested item");
            return Ok(None);
        }
    };

    if let Some(command) = command_option {
        match command.as_str() {
            "k" => backup_target(file, cli)?,
            "d" => {
                diff_files(&candidate_object.path, file);
                return Ok(None);
            }
            &_ => (),
        }
    };

    Ok(Some((candidate_object.path.clone(), file.to_path_buf())))
}

fn diff_files(source_file: &Path, target_file: &Path) {
    let mut cmd = Command::new(DIFF);
    cmd.arg(source_file).arg(target_file);
    match cmd.output() {
        Ok(out) => println!("{}", String::from_utf8_lossy(&out.stdout)),
        Err(e) => {
            eprintln!(
                "Failed to run `/bin/diff {}, {}`: {}",
                source_file.display(),
                target_file.display(),
                e
            );
            std::process::exit(3);
        }
    }
}

fn backup_target(src: &Path, cli: &Cli) -> io::Result<()> {
    let dest = src.with_extension("backup");

    if cli.verbose || cli.noop {
        println!("{} -> {}", src.display(), dest.display());
    }

    if cli.noop {
        Ok(())
    } else if dest.exists() {
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("Backup target {} exists", dest.display()),
        ))
    } else {
        fs::rename(src, dest)
    }
}

fn candidates(filesystem_root: &Path, file: &Path, cli: &Cli) -> io::Result<Candidates> {
    let snapshot_dirs = match all_snapshot_dirs(filesystem_root) {
        Some(dirs) => dirs,
        None => {
            eprintln!("No snapshots found under {}", filesystem_root.display());
            return Ok(Vec::new());
        }
    };

    if cli.verbose {
        println!("Found {} snapshots.", snapshot_dirs.len());
    }

    let relative_path = match path_relative_to_fs_root(file, filesystem_root) {
        Some(path) => path,
        None => {
            eprintln!(
                "Failed to calculate path for {} relative to {}",
                file.display(),
                filesystem_root.display()
            );
            return Ok(Vec::new());
        }
    };

    let ret: Candidates = snapshot_dirs
        .iter()
        .filter_map(|snapdir| {
            let candidate = snapdir.join(&relative_path);
            if cli.verbose {
                print!("{}: ", candidate.display());
            }
            if candidate.exists() {
                if cli.verbose {
                    println!("found candidate file");
                }
                match details_of(snapdir, &candidate) {
                    Ok(candidate) => Some(candidate),
                    Err(e) => {
                        eprintln!("Failed to get mtime for {}: {}", candidate.display(), e);
                        None
                    }
                }
            } else {
                if cli.verbose {
                    println!("no candidate file");
                }
                None
            }
        })
        .collect();

    Ok(ret)
}

fn details_of(snapdir: &Path, file: &Path) -> io::Result<Candidate> {
    let metadata = fs::metadata(file)?;

    let candidate = Candidate {
        snapname: snapdir.file_name().unwrap().to_string_lossy().to_string(),
        path: file.to_owned(),
        mtime: metadata.mtime(),
        size: metadata.size(),
    };

    Ok(candidate)
}

fn original_details(file: &Path) -> io::Result<Option<Candidate>> {
    let ret = if file.exists() {
        let metadata = fs::metadata(file)?;

        Some(Candidate {
            snapname: ".".to_string(),
            path: file.to_owned(),
            mtime: metadata.mtime(),
            size: metadata.size(),
        })
    } else {
        None
    };

    Ok(ret)
}

fn path_relative_to_fs_root(file: &Path, filesystem_root: &Path) -> Option<PathBuf> {
    file.strip_prefix(filesystem_root).ok().map(PathBuf::from)
}

// We need to canonicalize the source file, whether it exists or not.
fn canonical_file(file: PathBuf) -> io::Result<PathBuf> {
    if file.is_absolute() {
        return file.canonicalize();
    }

    let pwd = std::env::current_dir()?.canonicalize()?;

    Ok(pwd.join(file))
}

fn main() {
    let cli = Cli::parse();
    let mut errs = 0;

    let opts = ZpZrOpts {
        verbose: cli.verbose,
        noop: cli.noop,
        noclobber: cli.noclobber,
    };

    for file in &cli.file_list {
        let f = match canonical_file(PathBuf::from(file)) {
            Ok(file) => file,
            Err(e) => {
                eprintln!("Failed to canonicalize {}: {}", file, e);
                errs += 1;
                continue;
            }
        };

        match restore_action(&PathBuf::from(&f), &cli) {
            Ok(Some((src, dest))) => {
                if let Err(e) = file_copier::copy_file(&src, &dest, &opts) {
                    eprintln!("ERROR restoring {}: {}", &f.display(), e);
                    errs += 1;
                }
            }
            Ok(None) => (),
            Err(e) => {
                eprintln!("ERROR working out how to restore {}: {}", &f.display(), e);
                errs += 1;
            }
        }
    }

    if errs > 0 {
        eprintln!("Encountered {} errors", errs);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use common::spec_helper::fixture;
    use std::fs;
    use tempfile::tempdir;

    #[cfg(target_os = "illumos")]
    #[test]
    fn test_all_snapshot_dirs() {
        let result = all_snapshot_dirs(&fixture("")).unwrap();
        assert!(!result.is_empty());
        assert_eq!(None, all_snapshot_dirs(&PathBuf::from("/tmp")));
    }

    #[test]
    fn test_path_relative_to_fs_root() {
        assert_eq!(
            PathBuf::from("d/e/f"),
            path_relative_to_fs_root(&PathBuf::from("/a/b/c/d/e/f"), &PathBuf::from("/a/b/c"))
                .unwrap()
        );

        assert_eq!(
            None,
            path_relative_to_fs_root(&PathBuf::from("/a/b/c/d/e/f"), &PathBuf::from("/g/h/i"))
        );
    }

    #[test]
    fn test_candidates() {
        let cli = Cli {
            file_list: vec!["irrelevant_for_test".into()],
            verbose: false,
            noop: false,
            auto: true,
            noclobber: false,
        };

        let mut expected = vec![
            fixture(".zfs/snapshot/monday/file_in_both"),
            fixture(".zfs/snapshot/tuesday/file_in_both"),
        ];

        let mut actual = candidates(&fixture(""), &fixture("file_in_both"), &cli)
            .unwrap()
            .into_iter()
            .map(|c| c.path)
            .collect::<Vec<PathBuf>>();

        expected.sort();
        actual.sort();
        assert_eq!(expected, actual);

        assert_eq!(
            vec![fixture(".zfs/snapshot/monday/file_in_monday"),],
            candidates(&fixture(""), &fixture("file_in_monday"), &cli)
                .unwrap()
                .into_iter()
                .map(|c| c.path)
                .collect::<Vec<PathBuf>>()
        );

        assert!(candidates(&fixture(""), &fixture("file_in_neither"), &cli)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn test_restore_action_auto_mode() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test_file.txt");
        fs::write(&file_path, "test content").unwrap();

        let cli = Cli {
            file_list: vec![file_path.to_string_lossy().into()],
            verbose: false,
            noop: false,
            auto: true,
            noclobber: false,
        };

        let result = restore_action(&file_path, &cli);
        assert!(result.is_ok());

        if let Some((src, dest)) = result.unwrap() {
            assert_eq!(src, file_path);
            assert_eq!(dest, file_path);
        }
    }

    #[test]
    fn test_restore_action_no_candidates() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("nonexistent_file.txt");

        let cli = Cli {
            file_list: vec![file_path.to_string_lossy().into()],
            verbose: false,
            noop: false,
            auto: false,
            noclobber: false,
        };

        let result = restore_action(&file_path, &cli);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }
}
