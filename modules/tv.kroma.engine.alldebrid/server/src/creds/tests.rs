use super::*;

fn saved(key: &str) -> Saved {
    Saved {
        url: String::new(),
        key: key.into(),
    }
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

#[test]
fn forgetting_the_last_magnet_drops_the_latest_key_too() {
    let dir = kroma_module_sdk::testing::temp_dir("ad-creds-forget");
    let creds = Creds::new(dir.path());
    creds.remember(Some("7"), &saved("k")).unwrap();
    creds.remember(Some("8"), &saved("k")).unwrap();

    creds.forget("7");
    assert_eq!(creds.candidates("8"), [saved("k")]);
    assert!(!creds.knows("7"));

    creds.forget("8");
    assert!(creds.candidates("8").is_empty());
}

#[test]
fn forgetting_a_hostile_id_touches_nothing() {
    let dir = kroma_module_sdk::testing::temp_dir("ad-creds-forget-bad");
    let creds = Creds::new(dir.path());
    creds.remember(Some("7"), &saved("k")).unwrap();

    creds.forget("../latest");
    creds.forget("latest");

    assert_eq!(creds.candidates("7"), [saved("k")]);
}
