use byte_unit::Byte;
use common::constants::ZFS;
use std::{
    io,
    process::{exit, Command, Output},
    string::FromUtf8Error,
};

fn list_dataset_usage() -> io::Result<Output> {
    Command::new(ZFS)
        .arg("list")
        .arg("-t")
        .arg("all")
        .arg("-Ho")
        .arg("name,used,usedbydataset")
        .output()
}

#[derive(Debug, PartialEq)]
struct Dataset {
    byte_size: u64,
    format_size: String,
    name: String,
}

fn parse_dataset_line(line: &str) -> Option<Dataset> {
    let chunks: Vec<&str> = line.split_whitespace().collect();

    if chunks.len() != 3 {
        eprintln!("ERROR: failed to parse '{}'", line);
        return None;
    }

    let size = if chunks[2] == "-" {
        chunks[1]
    } else {
        chunks[2]
    };

    match Byte::parse_str(size, true) {
        Ok(byte_size) => {
            let byte_size = byte_size.as_u64();
            if byte_size == 0 {
                None
            } else {
                Some(Dataset {
                    byte_size,
                    format_size: size.to_string(),
                    name: chunks[0].to_string(),
                })
            }
        }
        Err(e) => {
            eprintln!("ERROR: failed to parse '{}': {}", line, e);
            None
        }
    }
}

fn parse_list_output(output: Output) -> Result<Vec<Dataset>, FromUtf8Error> {
    let stdout_string = String::from_utf8(output.stdout)?;

    let mut non_zero_datasets: Vec<Dataset> = stdout_string
        .lines()
        .filter_map(parse_dataset_line)
        .collect();

    non_zero_datasets.sort_by_key(|dataset| dataset.byte_size);
    Ok(non_zero_datasets)
}

fn display_list(sorted_dataset_list: Vec<Dataset>) {
    for dataset in sorted_dataset_list {
        println!("  {:>6}  {}", dataset.format_size, dataset.name);
    }
}

fn main() {
    match list_dataset_usage() {
        Ok(output) => match parse_list_output(output) {
            Ok(parsed_list) => display_list(parsed_list),
            Err(e) => {
                eprintln!("ERROR: failed to parse dataset list: {}", e);
                exit(2);
            }
        },
        Err(e) => {
            eprintln!("ERROR: failed to list datasets: {}", e);
            exit(1);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_line() {
        assert_eq!(
            Dataset {
                byte_size: 6050000000_u64,
                format_size: "6.05G".to_string(),
                name: "rpool/zones/serv-build/ROOT/zbe-3".to_string(),
            },
            parse_dataset_line("rpool/zones/serv-build/ROOT/zbe-3       6.13G   6.05G").unwrap()
        );

        assert_eq!(
            None,
            parse_dataset_line("fast/zone/build@03:00   0B      -")
        );
        assert_eq!(None, parse_dataset_line(""));
    }
}
