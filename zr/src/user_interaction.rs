use crate::types::{Candidate, Candidates, IoResult, UserChoice};
use colored::Colorize;
use regex::Regex;
use std::io::{self, Write};
use time::{format_description, OffsetDateTime, UtcOffset};

pub fn print_options(original_file: Option<Candidate>, candidates: &Candidates) {
    let mut stdout = io::stdout();
    for (index, candidate) in candidates.iter().enumerate() {
        let basic_line = basic_line(index, candidate);
        writeln!(
            stdout,
            "{}",
            decorated_line(&original_file, candidate, basic_line)
        )
        .unwrap();
    }
}

pub fn get_choice() -> IoResult<String> {
    print!("choose file to promote [add 'd' for diff, 'k' to keep] > ");
    io::stdout().flush().unwrap();
    let mut buffer = String::new();
    let stdin = io::stdin();
    stdin.read_line(&mut buffer)?;
    Ok(buffer.to_owned().trim().to_string())
}

pub fn parse_choice(input: &str) -> UserChoice {
    let pattern = Regex::new(r"^(\d+)([a-z]?)$").unwrap();
    let captures = pattern.captures(input)?;

    let number = captures.get(1)?.as_str().parse::<usize>().ok()?;
    let command = captures
        .get(2)
        .filter(|m| !m.as_str().is_empty())
        .map(|m| m.as_str().to_string());

    Some((number, command))
}

fn basic_line(index: usize, candidate: &Candidate) -> String {
    format!(
        "{:>2} {:<20} {:<35} {}",
        index,
        candidate.snapname,
        format_timestamp(candidate.mtime),
        candidate.size
    )
}

fn decorated_line(
    original_file: &Option<Candidate>,
    candidate_file: &Candidate,
    basic_line: String,
) -> String {
    if let Some(f) = original_file {
        if f.size == candidate_file.size && f.mtime == candidate_file.mtime {
            return basic_line.strikethrough().to_string();
        } else if f.size == candidate_file.size {
            return basic_line;
        }
        basic_line.blue().to_string()
    } else {
        basic_line
    }
}

fn format_timestamp(timestamp: i64) -> String {
    let datetime =
        OffsetDateTime::from_unix_timestamp(timestamp).unwrap_or(OffsetDateTime::UNIX_EPOCH);
    let local_offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
    let local_datetime = datetime.to_offset(local_offset);
    let format = format_description::parse(
        "[year]-[month]-[day] [hour]:[minute]:[second] [offset_hour sign:mandatory][offset_minute]",
    )
    .unwrap();

    local_datetime
        .format(&format)
        .unwrap_or_else(|_| String::from("Invalid date"))
}

#[cfg(test)]
mod test {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_basic_line() {
        let candidate = Candidate {
            snapname: "may".to_string(),
            path: PathBuf::from("some/path"),
            mtime: 1730563919,
            size: 150679,
        };

        assert_eq!(
            " 0 may                  2024-11-02 16:11:59 +0000           150679".to_string(),
            basic_line(0, &candidate)
        );
    }

    #[test]
    fn test_parse_choice() {
        assert_eq!(None, parse_choice("x"));
        assert_eq!(
            (47_usize, Some("k".to_string())),
            parse_choice("47k").unwrap()
        );
        assert_eq!((7_usize, None), parse_choice("7").unwrap());
    }
}
