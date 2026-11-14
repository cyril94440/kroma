use super::*;

fn saved(key: &str) -> Saved {
    Saved { url: String::new(), key: key.into() }
}

#[test]
fn a_magnet_finds_its_own_record_then_the_latest() {
    let dir = kroma_module_sdk::testing::temp_dir("ad-creds");
    let creds = Creds::new(dir.path());
    creds.remember(Some("7"), &saved("old")).unwrap();
    creds.remember(None, &saved("new")).unwrap();

    assert_eq!(creds.candidates("7"), [saved("old"), saved("new")]);
    assert_eq!(creds.candidates("8"), [saved("new")]);
}

#[test]
fn a_rotated_key_is_not_listed_twice() {
    let dir = kroma_module_sdk::testing::temp_dir("ad-creds-dup");
    let creds = Creds::new(dir.path());

    creds.remember(Some("7"), &saved("same")).unwrap();

    assert_eq!(creds.candidates("7"), [saved("same")]);
}

#[test]
fn nothing_remembered_is_nothing_found_and_a_hostile_id_reads_no_file() {
    let dir = kroma_module_sdk::testing::temp_dir("ad-creds-none");
    let creds = Creds::new(dir.path());

    assert!(creds.candidates("7").is_empty());
    assert!(creds.candidates("../latest").is_empty());
}

#[cfg(unix)]
#[test]
fn the_records_are_private_to_the_server_user() {
    use std::os::unix::fs::PermissionsExt;
    let dir = kroma_module_sdk::testing::temp_dir("ad-creds-mode");
    let creds = Creds::new(dir.path());

    creds.remember(Some("7"), &saved("k")).unwrap();

    let mode = |path: PathBuf| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(dir.path().join("credentials")), 0o700);
    assert_eq!(mode(dir.path().join("credentials/7.json")), 0o600);
}
