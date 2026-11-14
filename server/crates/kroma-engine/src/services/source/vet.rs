use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use kroma_http::address::WebTarget;
use kroma_primitives::input::RemoteUrl;
use kroma_primitives::remote_file::RemoteFile;

use super::locate::Located;
use super::{Refusal, SourceError};

pub(super) const PURPOSE: &str = "playback";

/// The folders a module may hold a title in: where each module's own data
/// directories sit, by module id.
pub type SourceRoots = Arc<dyn Fn(&str) -> Vec<PathBuf> + Send + Sync>;

/// Whether a module declared that it connects to a host, by module id.
pub type HostDeclared = Arc<dyn Fn(&str, &WebTarget) -> bool + Send + Sync>;

/// Where a stub's bytes are, once vetted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    Copy(LocalCopy),
    Upstream(RemoteUrl),
}

/// A complete file in a source's own folders, vetted on the handle that opened
/// it: what is read later is opened again and held to the same size and
/// identity, so a path swapped for a link or another file is not followed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalCopy {
    pub path: PathBuf,
    size: u64,
    identity: Option<(u64, u64)>,
}

impl LocalCopy {
    pub fn open(&self) -> std::io::Result<File> {
        let file = open_without_following(&self.path)?;
        let meta = file.metadata()?;
        if !meta.is_file() || meta.len() != self.size || identity_of(&meta) != self.identity {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "the copy is no longer the file that was vetted",
            ));
        }
        Ok(file)
    }

    /// Whether the file at the path is still the one that was vetted.
    pub(super) fn is_there(&self) -> bool {
        self.open().is_ok()
    }
}

#[cfg(unix)]
fn open_without_following(path: &Path) -> std::io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
}

#[cfg(not(unix))]
fn open_without_following(path: &Path) -> std::io::Result<File> {
    File::open(path)
}

#[cfg(unix)]
fn identity_of(meta: &std::fs::Metadata) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    Some((meta.dev(), meta.ino()))
}

#[cfg(not(unix))]
fn identity_of(_: &std::fs::Metadata) -> Option<(u64, u64)> {
    None
}

pub(super) struct Vetting {
    pub(super) declared: HostDeclared,
    pub(super) roots: SourceRoots,
}

impl Vetting {
    pub(super) fn vet(&self, located: &Located, stub: &RemoteFile) -> Result<Place, SourceError> {
        if let Some(path) = &located.path {
            return self
                .vet_path(&located.module_id, path, stub)
                .map(Place::Copy);
        }
        match &located.url {
            Some(url) => self.vet_url(&located.module_id, url).map(Place::Upstream),
            None => Err(SourceError::Refused(Refusal::NoPathNoUrl)),
        }
    }

    fn vet_path(
        &self,
        module: &str,
        path: &str,
        stub: &RemoteFile,
    ) -> Result<LocalCopy, SourceError> {
        let refused = || SourceError::Refused(Refusal::PathOutsideItsFolders);
        if !Path::new(path).is_absolute() {
            return Err(refused());
        }
        let canonical = Path::new(path).canonicalize().map_err(|_| refused())?;
        let held = (self.roots)(module)
            .iter()
            .filter_map(|root| root.canonicalize().ok())
            .any(|root| canonical.starts_with(root));
        if !held {
            return Err(refused());
        }
        let file = open_without_following(&canonical).map_err(|_| refused())?;
        let meta = file
            .metadata()
            .map_err(|_| SourceError::Refused(Refusal::NotTheFileStubbed))?;
        if !meta.is_file() || meta.len() != stub.size {
            return Err(SourceError::Refused(Refusal::NotTheFileStubbed));
        }
        Ok(LocalCopy {
            path: canonical,
            size: stub.size,
            identity: identity_of(&meta),
        })
    }

    fn vet_url(&self, module: &str, url: &str) -> Result<RemoteUrl, SourceError> {
        let parsed =
            RemoteUrl::parse(url).map_err(|why| SourceError::Refused(Refusal::Url(why)))?;
        let target =
            kroma_http::address::web_target(parsed.expose()).ok_or(SourceError::Refused(
                Refusal::Url(kroma_primitives::input::RemoteUrlError::NotAUrl),
            ))?;
        if !(self.declared)(module, &target) {
            return Err(SourceError::Refused(Refusal::HostNotDeclared));
        }
        Ok(parsed)
    }
}
