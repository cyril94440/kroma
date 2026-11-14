//! The credentials this module keeps so it can reach AllDebrid when the core
//! asks for a file without a client in hand: one record per magnet, and the
//! latest seen, so a rotated key keeps older magnets working.
//!
//! A component cannot set or read a file mode, so the protection is the folder
//! it lives in: the host creates this module's data directory owner-only before
//! the module starts and tightens it again if it was ever opened to others.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const LATEST: &str = "latest";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Saved {
    pub url: String,
    pub key: String,
}

#[derive(Debug, Clone)]
pub struct Creds {
    dir: PathBuf,
}

impl Creds {
    pub fn new(module_dir: &Path) -> Self {
        Self {
            dir: module_dir.join("credentials"),
        }
    }

    pub fn remember(&self, magnet: Option<&str>, saved: &Saved) -> std::io::Result<()> {
        self.write(LATEST, saved)?;
        match magnet {
            Some(id) => self.write(id, saved),
            None => Ok(()),
        }
    }

    /// The magnet's own record first, then the latest, without repeats.
    pub fn candidates(&self, magnet: &str) -> Vec<Saved> {
        let mut found: Vec<Saved> = Vec::new();
        let own = crate::ensure_magnet_id(magnet)
            .ok()
            .and_then(|()| self.read(magnet));
        for saved in own.into_iter().chain(self.read(LATEST)) {
            if !found.contains(&saved) {
                found.push(saved);
            }
        }
        found
    }

    pub fn knows(&self, magnet: &str) -> bool {
        self.read(magnet).is_some()
    }

    /// Drops the magnet's record, and the latest one too once no magnet keeps
    /// a record of its own: with nothing left to reach, no key is kept.
    pub fn forget(&self, magnet: &str) {
        if crate::ensure_magnet_id(magnet).is_err() {
            return;
        }
        let _ = std::fs::remove_file(self.path(magnet));
        if !self.holds_a_magnet() {
            let _ = std::fs::remove_file(self.path(LATEST));
        }
    }

    fn holds_a_magnet(&self) -> bool {
        std::fs::read_dir(&self.dir).is_ok_and(|entries| {
            entries.flatten().any(|entry| {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                name.strip_suffix(".json")
                    .is_some_and(|stem| stem != LATEST && crate::ensure_magnet_id(stem).is_ok())
            })
        })
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.json"))
    }

    fn read(&self, name: &str) -> Option<Saved> {
        serde_json::from_slice(&std::fs::read(self.path(name)).ok()?).ok()
    }

    fn write(&self, name: &str, saved: &Saved) -> std::io::Result<()> {
        if self.read(name).as_ref() == Some(saved) {
            return Ok(());
        }
        std::fs::create_dir_all(&self.dir)?;
        let target = self.path(name);
        let staged = self.dir.join(format!("{name}.tmp"));
        let mut file = std::fs::File::create(&staged)?;
        file.write_all(&serde_json::to_vec(saved).map_err(std::io::Error::other)?)?;
        file.flush()?;
        std::fs::rename(staged, target)
    }
}

#[cfg(test)]
mod tests;
