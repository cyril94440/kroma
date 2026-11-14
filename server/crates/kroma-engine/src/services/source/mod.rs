//! Titles held by a module rather than by this server. A library file that is a
//! [`kroma_primitives::remote_file`] stub names a `source`; asking that source
//! where the bytes are yields either a complete local copy or a short-lived
//! `https` URL, and this is the one place either is believed.
//!
//! Everything that reads a title's bytes asks [`Sources::resolve`] for a
//! [`MediaInput`] and never looks at the stub itself.

mod error;
mod point;
mod relay;
mod vet;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use tracing::warn;

use kroma_primitives::input::MediaInput;
use kroma_primitives::remote_file::{self, RemoteFile};

use crate::services::network::DeclaredPermissions;
use crate::state::Contributions;

pub use error::{Refusal, SourceError};
pub use point::{Located, POINT};
pub use kroma_media::stream::Upstream;
pub use kroma_primitives::remote_file::is_remote_file as is_stub;
pub use vet::SourceRoots;

use vet::Vetting;

const UNCACHED_REMEMBERED_FOR: Duration = Duration::from_secs(5 * 60);

/// A title's readable input, and the stub it came from when it had one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub input: MediaInput,
    pub stub: Option<RemoteFile>,
}

impl Resolved {
    /// The container the bytes really are: the stub's, or the file's extension.
    pub fn container(&self) -> Option<String> {
        match (&self.stub, &self.input) {
            (Some(stub), _) => stub.container(),
            (None, MediaInput::Local(path)) => path
                .extension()
                .and_then(std::ffi::OsStr::to_str)
                .map(str::to_ascii_lowercase),
            (None, MediaInput::Remote(_)) => None,
        }
    }
}

enum Memo {
    Cached(PathBuf, u64),
    Uncached(Instant),
}

pub struct Sources {
    contributions: Contributions,
    vetting: Vetting,
    memo: Mutex<HashMap<String, Memo>>,
}

impl Sources {
    pub fn new(contributions: Contributions, declared: DeclaredPermissions, roots: SourceRoots) -> Self {
        Self {
            contributions,
            vetting: Vetting::new(declared, roots),
            memo: Mutex::new(HashMap::new()),
        }
    }

    /// Accept plain `http` and loopback URLs from a source, so an integration
    /// test can serve a title from a socket on this machine. Compiled only with
    /// the `insecure-test-sources` feature, which no build of the server turns on.
    #[cfg(feature = "insecure-test-sources")]
    pub fn allow_loopback(&self) {
        self.vetting
            .insecure
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// The input holding the bytes of the library file at `abs`. A file that is
    /// not a stub is its own input; a stub is looked up at its source, which with
    /// `warm` also starts filling its local copy in the background. Blocking:
    /// a stub costs a call to a module. Run it on a blocking thread.
    pub fn resolve(&self, abs: &str, warm: bool) -> Result<Resolved, SourceError> {
        let path = Path::new(abs);
        if !remote_file::is_remote_file(path) {
            return Ok(Resolved {
                input: MediaInput::Local(path.to_path_buf()),
                stub: None,
            });
        }
        let stub = remote_file::read(path).map_err(|error| SourceError::BadStub(error.to_string()))?;
        let located = point::locate(&self.contributions, &stub.source, &stub.reference, warm)?;
        let input = self.vetting.vet(&located, &stub)?;
        if let MediaInput::Remote(url) = &input {
            tracing::debug!(host = url.host(), source = %stub.source, "a source answered a URL");
        }
        Ok(Resolved {
            input,
            stub: Some(stub),
        })
    }

    /// The file to read in full for the library file at `abs`, for work that
    /// must not stream a whole title: `abs` itself, or a stub's complete local
    /// copy. `None` while a stub's bytes are only a URL away. Never warms, and
    /// remembers a stub that has no copy yet for a few minutes so a pass over a
    /// library does not ask its source about every title again.
    pub fn local(&self, abs: &str) -> Result<Option<PathBuf>, SourceError> {
        if !remote_file::is_remote_file(Path::new(abs)) {
            return Ok(Some(PathBuf::from(abs)));
        }
        if let Some(known) = self.remembered(abs) {
            return Ok(known);
        }
        let found = match self.resolve(abs, false)? {
            Resolved { input: MediaInput::Local(path), stub: Some(stub) } => Some((path, stub.size)),
            _ => None,
        };
        let memo = match &found {
            Some((path, size)) => Memo::Cached(path.clone(), *size),
            None => Memo::Uncached(Instant::now()),
        };
        self.memo
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(abs.to_string(), memo);
        Ok(found.map(|(path, _)| path))
    }

    fn remembered(&self, abs: &str) -> Option<Option<PathBuf>> {
        let mut memo = self.memo.lock().unwrap_or_else(PoisonError::into_inner);
        let answer = match memo.get(abs)? {
            Memo::Cached(path, size) => std::fs::metadata(path)
                .is_ok_and(|meta| meta.is_file() && meta.len() == *size)
                .then(|| Some(path.clone())),
            Memo::Uncached(at) => (at.elapsed() < UNCACHED_REMEMBERED_FOR).then_some(None),
        };
        if answer.is_none() {
            memo.remove(abs);
        }
        answer
    }
}

/// Logs a failed resolution without anything a bearer link could be in.
pub fn log_failure(subject: &str, error: &SourceError) {
    warn!(%subject, %error, "a title's source could not be reached");
}
