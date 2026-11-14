//! The credentials this module keeps so it can reach AllDebrid when the core
//! asks for a file without a client in hand: one record per magnet, and the
//! latest seen, so a rotated key keeps older magnets working.

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
        restrict_dir(&self.dir);
        let target = self.path(name);
        let staged = self.dir.join(format!("{name}.tmp"));
        let mut file = create_private(&staged)?;
        file.write_all(&serde_json::to_vec(saved).map_err(std::io::Error::other)?)?;
        file.flush()?;
        std::fs::rename(staged, target)
    }
}

#[cfg(unix)]
fn create_private(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn create_private(path: &Path) -> std::io::Result<std::fs::File> {
    std::fs::File::create(path)
}

#[cfg(unix)]
fn restrict_dir(dir: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
}

#[cfg(not(unix))]
fn restrict_dir(_dir: &Path) {}

#[cfg(test)]
mod tests;
