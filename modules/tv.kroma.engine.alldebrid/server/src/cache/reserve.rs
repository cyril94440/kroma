use std::collections::HashSet;
use std::path::PathBuf;
use std::time::SystemTime;

use super::purge::{is_partial, Entry, GRACE};

pub const MAX_PULLS: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Load {
    pub pulls: usize,
    pub unwritten: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Room {
    Fits(Vec<PathBuf>),
    TooBig,
    Busy,
    Full,
}

/// Whether a pull of `need` more bytes fits under `budget`, counting every file
/// on disk (finished, `.part`, `.chunk`) and the bytes the running pulls have
/// still to write; and which finished, idle files to evict, least recently used
/// first, so it does.
pub fn room_for(
    entries: &[Entry],
    now: SystemTime,
    need: u64,
    file_size: u64,
    budget: u64,
    load: Load,
    running: &HashSet<PathBuf>,
) -> Room {
    if file_size > budget {
        return Room::TooBig;
    }
    if load.pulls >= MAX_PULLS {
        return Room::Busy;
    }
    let mut used: u64 = entries.iter().map(|entry| entry.size).sum::<u64>() + load.unwritten;
    let mut evictable: Vec<&Entry> = entries
        .iter()
        .filter(|entry| !is_partial(&entry.path) && !running.contains(&entry.path))
        .filter(|entry| now.duration_since(entry.last_used).unwrap_or_default() >= GRACE)
        .collect();
    evictable.sort_by_key(|entry| entry.last_used);
    let mut evict = Vec::new();
    for entry in evictable {
        if used + need <= budget {
            break;
        }
        used -= entry.size;
        evict.push(entry.path.clone());
    }
    if used + need <= budget {
        Room::Fits(evict)
    } else {
        Room::Full
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    const NOW_OFFSET: u64 = 1_000_000;

    fn now() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(NOW_OFFSET)
    }

    fn entry(name: &str, size: u64, idle_secs: u64) -> Entry {
        Entry {
            path: PathBuf::from(name),
            size,
            last_used: now() - Duration::from_secs(idle_secs),
        }
    }

    fn idle() -> Load {
        Load {
            pulls: 0,
            unwritten: 0,
        }
    }

    #[test]
    fn a_file_larger_than_the_whole_budget_is_never_started() {
        let room = room_for(&[], now(), 60, 60, 50, idle(), &HashSet::new());

        assert_eq!(room, Room::TooBig);
    }

    #[test]
    fn three_big_files_under_a_budget_for_one_start_one() {
        let first = room_for(&[], now(), 60, 60, 100, idle(), &HashSet::new());
        let second = room_for(
            &[entry("a.part", 10, 0)],
            now(),
            60,
            60,
            100,
            Load {
                pulls: 1,
                unwritten: 50,
            },
            &HashSet::new(),
        );

        assert_eq!(first, Room::Fits(vec![]));
        assert_eq!(second, Room::Full);
    }

    #[test]
    fn the_least_recently_used_finished_files_make_room() {
        let entries = [
            entry("old", 40, 10_000),
            entry("older", 40, 20_000),
            entry("fresh", 10, 5),
        ];

        let room = room_for(&entries, now(), 50, 50, 100, idle(), &HashSet::new());

        assert_eq!(room, Room::Fits(vec![PathBuf::from("older")]));
    }

    #[test]
    fn a_partial_file_a_running_one_and_a_recent_one_are_never_evicted() {
        let entries = [
            entry("big.part", 80, 20_000),
            entry("live", 10, 20_000),
            entry("recent", 10, 5),
        ];
        let running: HashSet<PathBuf> = [PathBuf::from("live")].into();

        let room = room_for(&entries, now(), 50, 50, 100, idle(), &running);

        assert_eq!(room, Room::Full);
    }

    #[test]
    fn the_pull_cap_answers_busy() {
        let load = Load {
            pulls: MAX_PULLS,
            unwritten: 0,
        };

        assert_eq!(
            room_for(&[], now(), 1, 1, 100, load, &HashSet::new()),
            Room::Busy
        );
    }
}
