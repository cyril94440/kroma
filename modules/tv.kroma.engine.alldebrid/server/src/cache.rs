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

pub use pull::Progress;
pub use purge::{Limits, Report};

pub const DIR: &str = "cache";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    Cached(PathBuf),
    Fetching { done: u64, total: u64, bps: u64 },
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct Cache {
    root: PathBuf,
}

impl Cache {
    pub fn new(module_dir: &Path) -> Self {
        Self { root: module_dir.join(DIR) }
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
    /// is already here. Needs a running async runtime.
    pub fn ensure_cached(&self, api: &Api, magnet: &str, file: &Planned) -> Result<Availability> {
        let path = self.path_of(magnet, &file.rel)?;
        if disk::is_complete(&path, file.size) {
            self.touch(&path);
            return Ok(Availability::Cached(path));
        }
        Ok(match pull::join_or_start(&path, file, api.clone()) {
            Progress::Running { done, total, bps } => Availability::Fetching { done, total, bps },
            Progress::Failed(reason) => Availability::Failed(reason),
        })
    }

    pub fn evict_magnet(&self, magnet: &str) -> Result<()> {
        let folder = self.root.join(magnet);
        if magnet.is_empty() || !magnet.bytes().all(|b| b.is_ascii_digit()) {
            bail!("not an AllDebrid magnet id");
        }
        pull::forget_under(&folder);
        match std::fs::remove_dir_all(&folder) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error.into()),
            _ => Ok(()),
        }
    }

    pub fn purge(&self, limits: Limits) -> Report {
        purge::purge(&self.root, SystemTime::now(), limits, &pull::running())
    }
}

#[cfg(test)]
mod tests;
