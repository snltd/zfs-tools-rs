use std::process::Command;

/// Returns a printable string of the given command
///
pub fn format_command(cmd: &Command) -> String {
    format!(
        "{} {}",
        cmd.get_program().to_string_lossy(),
        cmd.get_args()
            .map(|arg| arg.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ")
    )
}

/// Takes a Command output and returns it as a Vec of strings. Empty lines
/// are omitted.
///
pub fn output_as_lines(mut cmd: Command) -> anyhow::Result<Vec<String>> {
    let raw_output = cmd.output()?;
    let string_output = String::from_utf8(raw_output.stdout)?;
    let lines: Vec<String> = string_output.lines().map(String::from).collect();

    Ok(lines)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_output_as_lines() {
        assert_eq!(
            Vec::<String>::new(),
            output_as_lines(Command::new("/bin/true")).unwrap()
        );

        let expected: Vec<String> = vec![
            "Cargo.toml".to_string(),
            "src".to_string(),
            "test".to_string(),
        ];

        assert_eq!(expected, output_as_lines(Command::new("/bin/ls")).unwrap());
    }
}
