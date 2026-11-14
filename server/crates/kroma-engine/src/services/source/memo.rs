use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use kroma_primitives::remote_file::RemoteFile;

use super::vet::Place;
use super::SourceError;

const UPSTREAM_FRESH: Duration = Duration::from_secs(30);
const UNCACHED_REMEMBERED: Duration = Duration::from_secs(5 * 60);
const FAILURE_REMEMBERED: Duration = Duration::from_secs(30);
const WARM_LIFETIME: Duration = Duration::from_secs(30 * 60);
const WARMS_IN_FLIGHT: usize = 3;

struct Entry {
    stub: RemoteFile,
    place: Place,
    at: Instant,
    warmed: bool,
}

enum Quiet {
    Uncached(Instant),
    Failed(Instant, SourceError),
}

/// What the core remembers about where a stub's bytes are, so that a player's
/// hundreds of range requests, or a pass over a library, ask a module once.
#[derive(Default)]
pub(super) struct Memo {
    entries: Mutex<HashMap<String, Entry>>,
    quiet: Mutex<HashMap<String, Quiet>>,
    warming: Mutex<HashMap<String, Instant>>,
}

fn held<T>(lock: &Mutex<T>) -> MutexGuard<'_, T> {
    lock.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Memo {
    /// The place `stub` was last vetted to be at, while that still holds: a
    /// local copy as long as it is there, a link for half a minute, and one that
    /// was not asked to warm never answers a call that is.
    pub(super) fn recall(&self, abs: &str, stub: &RemoteFile, warm: bool) -> Option<Place> {
        let mut entries = held(&self.entries);
        let entry = entries.get(abs)?;
        let current = entry.stub == *stub
            && match &entry.place {
                Place::Copy(copy) => copy.is_there(),
                Place::Upstream(_) => entry.at.elapsed() < UPSTREAM_FRESH && (entry.warmed || !warm),
            };
        if current {
            return Some(entry.place.clone());
        }
        entries.remove(abs);
        None
    }

    pub(super) fn remember(&self, abs: &str, stub: RemoteFile, place: Place, warmed: bool) {
        if matches!(place, Place::Copy(_)) {
            held(&self.quiet).remove(abs);
            held(&self.warming).remove(abs);
        }
        held(&self.entries).insert(abs.to_string(), Entry { stub, place, at: Instant::now(), warmed });
    }

    pub(super) fn forget(&self, abs: &str) {
        held(&self.entries).remove(abs);
    }

    pub(super) fn note_uncached(&self, abs: &str) {
        held(&self.quiet).insert(abs.to_string(), Quiet::Uncached(Instant::now()));
    }

    pub(super) fn note_failure(&self, abs: &str, error: &SourceError) {
        if error.is_transient() {
            held(&self.quiet).insert(abs.to_string(), Quiet::Failed(Instant::now(), error.clone()));
        }
    }

    /// `Some(Ok(()))` for a stub that had no copy a moment ago, `Some(Err(..))`
    /// for one whose source failed a moment ago.
    pub(super) fn recently(&self, abs: &str) -> Option<Result<(), SourceError>> {
        let mut quiet = held(&self.quiet);
        let answer = match quiet.get(abs)? {
            Quiet::Uncached(at) => (at.elapsed() < UNCACHED_REMEMBERED).then_some(Ok(())),
            Quiet::Failed(at, error) => (at.elapsed() < FAILURE_REMEMBERED).then(|| Err(error.clone())),
        };
        if answer.is_none() {
            quiet.remove(abs);
        }
        answer
    }

    /// Takes one of the few places a probe may ask a source to start caching a
    /// title. A title already being warmed keeps its place until its copy lands
    /// or the warm is old enough to have failed.
    pub(super) fn may_warm(&self, abs: &str) -> bool {
        let mut warming = held(&self.warming);
        warming.retain(|_, at| at.elapsed() < WARM_LIFETIME);
        if warming.contains_key(abs) {
            return true;
        }
        if warming.len() >= WARMS_IN_FLIGHT {
            return false;
        }
        warming.insert(abs.to_string(), Instant::now());
        true
    }
}
