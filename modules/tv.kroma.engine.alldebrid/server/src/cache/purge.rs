use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::disk;

pub const GRACE: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub max_bytes: u64,
    pub max_age: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub path: PathBuf,
    pub size: u64,
    pub last_used: SystemTime,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub removed: usize,
    pub freed: u64,
}

/// The entries to delete: unprotected ones idle past `max_age`, then the least
/// recently used unprotected ones until the rest fits `max_bytes`. A zero limit
/// switches that limit off. An entry being fetched, or used within [`GRACE`],
/// is never chosen.
pub fn victims(
    entries: &[Entry],
    now: SystemTime,
    limits: Limits,
    running: &HashSet<PathBuf>,
) -> Vec<PathBuf> {
    let idle = |entry: &Entry| now.duration_since(entry.last_used).unwrap_or_default();
    let protected = |entry: &Entry| running.contains(&entry.path) || idle(entry) < GRACE;
    let mut chosen = Vec::new();
    let mut kept: Vec<&Entry> = Vec::new();
    for entry in entries {
        let expired = !limits.max_age.is_zero() && idle(entry) > limits.max_age;
        if expired && !protected(entry) {
            chosen.push(entry.path.clone());
        } else {
            kept.push(entry);
        }
    }
    if limits.max_bytes > 0 {
        let mut total: u64 = kept.iter().map(|entry| entry.size).sum();
        let mut evictable: Vec<&Entry> = kept.into_iter().filter(|entry| !protected(entry)).collect();
        evictable.sort_by_key(|entry| entry.last_used);
        for entry in evictable {
            if total <= limits.max_bytes {
                break;
            }
            total -= entry.size;
            chosen.push(entry.path.clone());
        }
    }
    chosen
}

pub fn entries(root: &Path) -> Vec<Entry> {
    let mut found = Vec::new();
    collect(root, &mut found);
    found
}

fn collect(dir: &Path, found: &mut Vec<Entry>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for item in read.flatten() {
        let path = item.path();
        let Ok(meta) = item.metadata() else {
            continue;
        };
        if meta.is_dir() {
            collect(&path, found);
        } else if meta.is_file() {
            found.push(Entry {
                path,
                size: meta.len(),
                last_used: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            });
        }
    }
}

pub fn purge(root: &Path, now: SystemTime, limits: Limits, running: &HashSet<PathBuf>) -> Report {
    let all = entries(root);
    let mut report = Report::default();
    for path in victims(&all, now, limits, running) {
        let size = disk::len(&path).unwrap_or(0);
        if std::fs::remove_file(&path).is_ok() {
            report.removed += 1;
            report.freed += size;
        }
    }
    remove_empty_dirs(root, true);
    report
}

fn remove_empty_dirs(dir: &Path, is_root: bool) -> bool {
    let Ok(read) = std::fs::read_dir(dir) else {
        return false;
    };
    let mut empty = true;
    for item in read.flatten() {
        let path = item.path();
        let child_empty = item.metadata().is_ok_and(|meta| meta.is_dir()) && remove_empty_dirs(&path, false);
        empty &= child_empty;
    }
    if empty && !is_root {
        return std::fs::remove_dir(dir).is_ok();
    }
    empty
}

#[cfg(test)]
mod tests;
