use crate::types::ZpZrOpts;
use std::fs;
use std::io;
use std::path::Path;

/// Recursively copies directory trees. Is able to merge with existing targets if opts.noclobber
/// is set.
pub fn copy_file(src: &Path, dest: &Path, opts: &ZpZrOpts) -> io::Result<u64> {
    if src.is_file() {
        copy_file_action(src, dest, opts)
    } else {
        if !dest.exists() {
            fs::create_dir_all(dest)?;
        }

        for f in fs::read_dir(src)? {
            let f = f?;
            let src_path = f.path();
            let dest_path = dest.join(f.file_name());

            if src.is_file() {
                copy_file_action(&src_path, &dest_path, opts)?;
            } else {
                copy_file(&src_path, &dest_path, opts)?;
            }
        }

        Ok(0)
    }
}

fn copy_file_action(src: &Path, dest: &Path, opts: &ZpZrOpts) -> io::Result<u64> {
    if dest.exists() && opts.noclobber {
        if opts.verbose {
            println!("{} exists and noclobber is set", dest.display());
        }
        Ok(0)
    } else {
        if opts.verbose || opts.noop {
            println!("{} -> {}", src.display(), dest.display());
        }

        if opts.noop || (src.is_dir() && dest.exists()) {
            Ok(0)
        } else {
            fs::copy(src, dest)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_copy_file_with_noclobber() {
        let tmp = tempdir().unwrap();
        let src = tmp.path().join("src.txt");
        let dest = tmp.path().join("dest.txt");

        fs::write(&src, "blah blah blah").unwrap();
        fs::write(&dest, "please don't clobber me!").unwrap();

        let opts = ZpZrOpts {
            verbose: false,
            noop: false,
            noclobber: true,
        };

        assert!(copy_file(&src, &dest, &opts).is_ok());
        assert_eq!(
            "please don't clobber me!",
            fs::read_to_string(&dest).unwrap()
        );
    }

    #[test]
    fn test_copy_file_without_noclobber() {
        let tmp = tempdir().unwrap();
        let src = tmp.path().join("src.txt");
        let dest = tmp.path().join("dest.txt");

        fs::write(&src, "it's clobbering time").unwrap();
        fs::write(&dest, "blah blah blah").unwrap();

        let opts = ZpZrOpts {
            verbose: false,
            noop: false,
            noclobber: false,
        };

        assert!(copy_file(&src, &dest, &opts).is_ok());
        assert_eq!("it's clobbering time", fs::read_to_string(&dest).unwrap());
    }

    #[test]
    fn test_copy_file_with_noop() {
        let tmp = tempdir().unwrap();
        let src = tmp.path().join("src.txt");
        let dest = tmp.path().join("dest.txt");

        fs::write(&src, "blah blah blah").unwrap();

        let opts = ZpZrOpts {
            verbose: false,
            noop: true,
            noclobber: false,
        };

        assert!(copy_file(&src, &dest, &opts).is_ok());
        assert!(!dest.exists());
    }

    #[test]
    fn test_copy_directory_with_files() {
        let tmp = tempdir().unwrap();
        let src_dir = tmp.path().join("src_dir");
        let dest_dir = tmp.path().join("dest_dir");

        fs::create_dir(&src_dir).unwrap();
        let src = src_dir.join("file.txt");
        fs::write(&src, "blah blah blah").unwrap();

        let opts = ZpZrOpts {
            verbose: false,
            noop: false,
            noclobber: false,
        };

        let dest = dest_dir.join("file.txt");

        assert!(copy_file(&src_dir, &dest_dir, &opts).is_ok());
        let dest_content = fs::read_to_string(&dest).unwrap();
        assert!(dest.exists());
        assert_eq!(dest_content, "blah blah blah");
    }

    #[test]
    fn test_copy_file_action_verbose() {
        let tmp = tempdir().unwrap();
        let src = tmp.path().join("src.txt");
        let dest = tmp.path().join("dest.txt");

        fs::write(&src, "blah blah blah").unwrap();

        let opts = ZpZrOpts {
            verbose: true,
            noop: false,
            noclobber: false,
        };

        assert!(copy_file_action(&src, &dest, &opts).is_ok());
        assert!(dest.exists());
        let dest_content = fs::read_to_string(&dest).unwrap();
        assert_eq!(dest_content, "blah blah blah");
    }
}
