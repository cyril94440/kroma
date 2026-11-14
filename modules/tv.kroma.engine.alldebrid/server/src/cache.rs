//! A cache of single files fetched from AllDebrid onto this server, which
//! purges itself.
//!
//! A component has one thread, and a call that holds it blocks every other call
//! to the module. So a fetch is a task the module leaves running, which the
//! host advances on its tick, and each step is one ranged request of at most
//! [`job::CHUNK_BYTES`]. What is running lives in memory, keyed by file; what is
//! done lives on disk, so an instance that restarts resumes at the byte it had
//! reached, and a file is only ever named by its final path once complete.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{bail, Result};

use crate::api::Api;
use crate::files::Planned;

mod disk;
mod job;
mod pull;
pub mod purge;
mod reserve;

pub use disk::dest as within;
pub use pull::{Progress, Tidy};
pub use purge::{Limits, Report};
pub use reserve::{Load, Room, MAX_PULLS};

pub const DIR: &str = "cache";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    Cached(PathBuf),
    Fetching { done: u64, total: u64, bps: u64 },
    Failed(String),
    Declined(String),
}

#[derive(Debug, Clone)]
pub struct Cache {
    root: PathBuf,
}

impl Cache {
    pub fn new(module_dir: &Path) -> Self {
        Self {
            root: module_dir.join(DIR),
        }
    }

    pub fn path_of(&self, magnet: &str, rel: &str) -> Result<PathBuf> {
        if magnet.is_empty() || !magnet.bytes().all(|b| b.is_ascii_digit()) {
            bail!("not an AllDebrid magnet id");
        }
        let clean = rel
            .split('/')
            .map(crate::files::component)
            .collect::<Vec<_>>()
            .join("/");
        if clean != rel {
            bail!("not a path this cache holds");
        }
        Ok(disk::dest(&self.root.join(magnet), rel))
    }

    /// The file's final path, only when it is complete and its size matches.
    pub fn cached_path(&self, magnet: &str, file: &Planned) -> Option<PathBuf> {
        let path = self.path_of(magnet, &file.rel).ok()?;
        disk::is_complete(&path, file.size).then_some(path)
    }

    pub fn touch(&self, path: &Path) {
        disk::mark_used(path, SystemTime::now());
    }

    /// Starts or joins the background fetch of one file, and touches it when it
    /// is already here. A fetch only starts when the file fits `limits` once the
    /// finished files, the partial ones and the fetches running are counted,
    /// evicting the least recently used finished files to make room; otherwise
    /// it is declined and playback carries on by relay. Needs a running async
    /// runtime.
    pub fn ensure_cached(
        &self,
        api: &Api,
        magnet: &str,
        file: &Planned,
        limits: Limits,
    ) -> Result<Availability> {
        let path = self.path_of(magnet, &file.rel)?;
        if disk::is_complete(&path, file.size) {
            self.touch(&path);
            return Ok(Availability::Cached(path));
        }
        if !pull::is_running(&path) {
            if let Some(reason) = self.make_room(&path, file.size, limits) {
                return Ok(Availability::Declined(reason));
            }
        }
        let tidy = Tidy {
            root: self.root.clone(),
            limits,
        };
        Ok(
            match pull::join_or_start(&path, file, api.clone(), Some(tidy)) {
                Progress::Running { done, total, bps } => {
                    Availability::Fetching { done, total, bps }
                }
                Progress::Failed(reason) => Availability::Failed(reason),
            },
        )
    }

    fn make_room(&self, dest: &Path, size: u64, limits: Limits) -> Option<String> {
        let budget = limits.max_bytes?;
        let held = disk::len(&disk::part_of(dest)).unwrap_or(0);
        let entries = purge::entries(&self.root);
        let room = reserve::room_for(
            &entries,
            SystemTime::now(),
            size.saturating_sub(held),
            size,
            budget,
            pull::load_under(&self.root),
            &pull::running(),
        );
        match room {
            Room::Fits(evict) => {
                for path in evict {
                    let _ = std::fs::remove_file(path);
                }
                None
            }
            Room::TooBig => Some(format!("{size} bytes is more than the {budget} the cache may hold")),
            Room::Busy => Some("the cache is already fetching as many files as it may".into()),
            Room::Full => Some("no room in the cache without dropping files in use".into()),
        }
    }

    /// Like [`Cache::ensure_cached`] for a file that belongs at `dest` instead:
    /// it is not part of the cache, so nothing purges it and nothing touches it.
    pub fn ensure_at(api: &Api, dest: &Path, file: &Planned) -> Availability {
        if disk::is_complete(dest, file.size) {
            return Availability::Cached(dest.to_path_buf());
        }
        match pull::join_or_start(dest, file, api.clone(), None) {
            Progress::Running { done, total, bps } => Availability::Fetching { done, total, bps },
            Progress::Failed(reason) => Availability::Failed(reason),
        }
    }

    pub fn stop_fetching_under(folder: &Path) {
        pull::forget_under(folder);
    }

    pub fn purge(&self, limits: Limits) -> Report {
        purge::purge(&self.root, SystemTime::now(), limits, &pull::running())
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests;
#[cfg(all(test, not(target_family = "wasm")))]
mod tests_edges;
