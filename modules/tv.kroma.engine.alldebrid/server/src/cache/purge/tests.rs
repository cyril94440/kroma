use std::time::Duration;

use super::*;

const DAY: Duration = Duration::from_secs(86_400);

fn at(now: SystemTime, idle: Duration) -> SystemTime {
    now - idle
}

fn entry(name: &str, size: u64, last_used: SystemTime) -> Entry {
    Entry { path: PathBuf::from(name), size, last_used }
}

fn limits(max_bytes: u64, days: u64) -> Limits {
    Limits { max_bytes, max_age: DAY * days as u32 }
}

fn names(paths: Vec<PathBuf>) -> Vec<String> {
    let mut names: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
    names.sort();
    names
}

#[test]
fn a_file_idle_past_the_age_limit_goes() {
    let now = SystemTime::now();
    let all = [entry("old", 1, at(now, DAY * 15)), entry("fresh", 1, at(now, DAY * 2))];

    assert_eq!(names(victims(&all, now, limits(0, 14), &HashSet::new())), ["old"]);
}

#[test]
fn the_least_recently_used_go_first_until_the_rest_fits() {
    let now = SystemTime::now();
    let all = [
        entry("c", 40, at(now, DAY * 3)),
        entry("a", 40, at(now, DAY * 5)),
        entry("b", 40, at(now, DAY * 4)),
    ];

    assert_eq!(names(victims(&all, now, limits(80, 0), &HashSet::new())), ["a"]);
    assert_eq!(names(victims(&all, now, limits(50, 0), &HashSet::new())), ["a", "b"]);
}

#[test]
fn a_file_used_within_the_grace_is_never_chosen_even_over_budget() {
    let now = SystemTime::now();
    let all = [entry("busy", 100, at(now, Duration::from_secs(60))), entry("idle", 10, at(now, DAY))];

    assert_eq!(names(victims(&all, now, limits(1, 0), &HashSet::new())), ["idle"]);
}

#[test]
fn a_file_being_fetched_is_never_chosen() {
    let now = SystemTime::now();
    let all = [entry("a.part", 100, at(now, DAY * 90)), entry("b", 100, at(now, DAY * 90))];
    let running = HashSet::from([PathBuf::from("a.part")]);

    assert_eq!(names(victims(&all, now, limits(1, 14), &running)), ["b"]);
}

#[test]
fn protected_bytes_still_count_against_the_budget() {
    let now = SystemTime::now();
    let all = [entry("busy", 90, at(now, Duration::from_secs(5))), entry("idle", 20, at(now, DAY))];

    assert_eq!(names(victims(&all, now, limits(100, 0), &HashSet::new())), ["idle"]);
}

#[test]
fn zero_limits_keep_everything() {
    let now = SystemTime::now();
    let all = [entry("a", 1 << 40, at(now, DAY * 900))];

    assert!(victims(&all, now, limits(0, 0), &HashSet::new()).is_empty());
}

#[test]
fn a_last_use_in_the_future_counts_as_just_used() {
    let now = SystemTime::now();
    let all = [entry("a", 100, now + DAY)];

    assert!(victims(&all, now, limits(1, 1), &HashSet::new()).is_empty());
}

#[test]
fn purging_a_folder_deletes_the_victims_and_the_directories_they_emptied() {
    let dir = kroma_module_sdk::testing::temp_dir("ad-purge");
    let root = dir.path();
    let write = |rel: &str, bytes: usize, idle: Duration| {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, vec![0u8; bytes]).unwrap();
        let file = std::fs::File::options().write(true).open(&path).unwrap();
        file.set_modified(SystemTime::now() - idle).unwrap();
    };
    write("7/old/a.mkv", 10, DAY * 20);
    write("7/keep/b.mkv", 10, DAY * 1);
    write("8/c.mkv.part", 10, DAY * 20);
    let running = HashSet::from([root.join("8/c.mkv.part")]);

    let report = purge(root, SystemTime::now(), limits(0, 14), &running);

    assert_eq!(report, Report { removed: 1, freed: 10 });
    assert!(!root.join("7/old").exists(), "emptied directory removed");
    assert!(root.join("7/keep/b.mkv").exists());
    assert!(root.join("8/c.mkv.part").exists());
    assert!(root.exists());
}
