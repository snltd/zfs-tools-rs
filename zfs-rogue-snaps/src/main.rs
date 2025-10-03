use common::zfs_info;
use regex::Regex;

fn filter_fn(snapshot: &String, expected: &[String], regex: &Regex) -> Option<String> {
    if let Some((fs, snap)) = snapshot.split_once("@")
        && !fs.starts_with("rpool/VARSHARE/zones")
        && !fs.starts_with("rpool/ROOT")
        && snap != "initial"
        && !(regex.is_match(snap))
        && !(expected.iter().any(|x| x == snap))
    {
        return Some(snapshot.to_string());
    }
    None
}

fn find_rogue_snapshots(snapshot_list: Vec<String>, expected_list: &[String]) -> Vec<String> {
    let regex = Regex::new(r"^[012]\d:[0-5]\d$").expect("invalid regex");
    snapshot_list
        .into_iter()
        .filter_map(|snap| filter_fn(&snap, expected_list, &regex))
        .collect()
}

fn main() {
    let defaults: Vec<String> = vec![
        "monday".to_string(),
        "tuesday".to_string(),
        "wednesday".to_string(),
        "thursday".to_string(),
        "friday".to_string(),
        "saturday".to_string(),
        "sunday".to_string(),
        "january".to_string(),
        "february".to_string(),
        "march".to_string(),
        "april".to_string(),
        "may".to_string(),
        "june".to_string(),
        "july".to_string(),
        "august".to_string(),
        "september".to_string(),
        "october".to_string(),
        "november".to_string(),
        "december".to_string(),
    ];

    let all_snapshots = match zfs_info::all_snapshots() {
        Ok(list) => list,
        Err(e) => {
            eprintln!("Failed to get snapshot list: {}", e);
            std::process::exit(1);
        }
    };

    let rogues = find_rogue_snapshots(all_snapshots, &defaults);
    print_rogues(rogues);
}

fn print_rogues(snaps: Vec<String>) {
    for snap in snaps {
        println!("{}", snap);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_find_rogue_snapshots() {
        let all_snapshots = vec![
            "rpool/ROOT@rogue".to_string(),
            "rpool@wednesday".to_string(),
            "rpool@rogue".to_string(),
            "rpool/VARSHARE/zones/zone@rogue".to_string(),
            "zones/myzone@initial".to_string(),
            "fast/zone/build/build@12:00".to_string(),
            "rpool/zones@october".to_string(),
            "fast/zone/build@99:99".to_string(),
        ];

        let defaults = vec!["wednesday".to_string(), "october".to_string()];

        assert_eq!(
            vec![
                "rpool@rogue".to_string(),
                "fast/zone/build@99:99".to_string()
            ],
            find_rogue_snapshots(all_snapshots, &defaults)
        );
    }
}
