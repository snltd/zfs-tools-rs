//! Functions, constants, and whatever else comes along, which are required by
//! more than one of the tools in this crate.
//!
pub mod utils {
    use std::error::Error;
    use std::process::Command;

    pub const ZFS: &str = "/usr/sbin/zfs";

    /// Returns a Vec of all the snapshots zfs can see, each being a string.
    ///
    pub fn all_snapshots() -> Result<Vec<String>, Box<dyn Error>> {
        let mut cmd = Command::new(ZFS);
        cmd.arg("list")
            .arg("-Ho")
            .arg("name")
            .arg("-t")
            .arg("snapshot");

        output_as_lines(cmd)
    }

    /// Returns a Vec of all mounted ZFS filesystems, described as Strings.
    ///
    pub fn all_zfs_mounts() -> Result<Vec<String>, Box<dyn Error>> {
        let mut cmd = Command::new(ZFS);
        cmd.arg("list").arg("-Ho").arg("mountpoint,name");
        output_as_lines(cmd)
    }

    /// Takes a Command output and returns it as a Vec of strings. Empty lines
    /// are omitted.
    ///
    pub fn output_as_lines(mut cmd: Command) -> Result<Vec<String>, Box<dyn Error>> {
        let raw_output = cmd.output()?;
        let string_output = String::from_utf8(raw_output.stdout)?;
        let lines: Vec<String> = string_output.lines().map(String::from).collect();

        Ok(lines)
    }
}

#[cfg(test)]
mod test {
    use super::utils::*;
    use std::process::Command;

    #[test]
    fn test_output_as_lines() {
        assert_eq!(
            Vec::<String>::new(),
            output_as_lines(Command::new("/bin/true")).unwrap()
        );

        let expected: Vec<String> = vec!["Cargo.toml".to_string(), "src".to_string()];

        assert_eq!(expected, output_as_lines(Command::new("/bin/ls")).unwrap());
    }
}
