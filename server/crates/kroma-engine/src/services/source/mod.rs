//! Titles held by a module rather than by this server. A library file that is a
//! [`kroma_primitives::remote_file`] stub names a `source`; asking that source
//! where the bytes are yields either a complete local copy or a short-lived
//! `https` URL, and this is the one place either is believed.
//!
//! Direct play reads a title through [`Sources::serve`]. Everything that runs
//! ffmpeg asks [`Sources::resolve`] for a [`MediaInput`] and never looks at the
//! stub itself: a copy is a file, and a URL is only ever read by the core, so
//! ffmpeg gets a grant on the loopback relay instead of an address.

mod error;
mod locate;
mod loopback;
mod memo;
mod relay;
mod serve;
mod vet;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};

use tracing::warn;

use kroma_primitives::input::{MediaInput, Relayed};
use kroma_primitives::remote_file::{self, RemoteFile};

use crate::state::{Contributions, OfficialModules};

pub use error::{Refusal, SourceError};
pub use kroma_media::stream::Upstream;
pub use kroma_primitives::remote_file::is_remote_file as is_stub;
pub use locate::{Located, POINT};
pub use loopback::Grant;
pub use relay::{link_is_stale, UpstreamStatus};
pub use serve::ServeError;
pub use vet::{HostDeclared, LocalCopy, Place, SourceRoots};

use memo::Memo;
use vet::Vetting;

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

/// Why a library file cannot be read right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unplayable {
    /// A file that is not there: its mount is offline, or it was removed.
    Missing,
    Source(SourceError),
}

/// What a stub's source said about it, vetted.
#[derive(Debug, Clone)]
pub struct Found {
    pub stub: RemoteFile,
    pub place: Place,
}

pub struct Sources {
    me: Weak<Sources>,
    contributions: Contributions,
    official: OfficialModules,
    vetting: Vetting,
    memo: Memo,
    relay: loopback::Loopback,
}

impl Sources {
    pub fn new(
        contributions: Contributions,
        official: OfficialModules,
        declared: HostDeclared,
        roots: SourceRoots,
    ) -> Arc<Self> {
        Self::with_relay(
            contributions,
            official,
            declared,
            roots,
            loopback::Loopback::new(),
        )
    }

    fn with_relay(
        contributions: Contributions,
        official: OfficialModules,
        declared: HostDeclared,
        roots: SourceRoots,
        relay: loopback::Loopback,
    ) -> Arc<Self> {
        Arc::new_cyclic(|me| Self {
            me: me.clone(),
            contributions,
            official,
            vetting: Vetting { declared, roots },
            memo: Memo::default(),
            relay,
        })
    }

    /// Where the library file at `abs` is held, as its source last said and the
    /// core vetted. A stub asks a module at most once per short window, a window
    /// in which `warm` reaches the module at most once; a complete local copy
    /// answers as soon as it exists. Blocking: run it on a blocking thread.
    pub fn find(&self, abs: &str, warm: bool) -> Result<Found, SourceError> {
        let stub = remote_file::read(Path::new(abs))
            .map_err(|error| SourceError::BadStub(error.to_string()))?;
        if let Some(place) = self.memo.recall(abs, &stub, warm) {
            return Ok(Found { stub, place });
        }
        let located = locate::locate(
            &self.contributions,
            &self.official,
            &stub.source,
            &stub.reference,
            warm,
        )?;
        let place = self.vetting.vet(&located, &stub)?;
        self.memo.remember(abs, stub.clone(), place.clone(), warm);
        Ok(Found { stub, place })
    }

    /// The input holding the bytes of the library file at `abs`, for ffmpeg and
    /// ffprobe. A file that is not a stub is its own input; a stub is a copy on
    /// this machine, or a grant on the loopback relay where this ffmpeg can read
    /// one. Where it cannot, a title with no copy is [`SourceError::Preparing`]
    /// and, with `warm`, its source starts caching it. Blocking.
    pub fn resolve(&self, abs: &str, warm: bool) -> Result<Resolved, SourceError> {
        let path = Path::new(abs);
        if !remote_file::is_remote_file(path) {
            return Ok(Resolved {
                input: MediaInput::Local(path.to_path_buf()),
                stub: None,
            });
        }
        let found = self.find(abs, warm)?;
        self.input_of(abs, found, kroma_media::capabilities::network_input())
    }

    /// What a route reads for the library file at `abs`: [`Self::resolve`], once a
    /// file that is not a stub is known to be there, so that an offline mount and
    /// a source that is down are told apart. Blocking.
    pub fn playable(&self, abs: &str, warm: bool) -> Result<Resolved, Unplayable> {
        let path = Path::new(abs);
        if !remote_file::is_remote_file(path) && !path.exists() {
            return Err(Unplayable::Missing);
        }
        self.resolve(abs, warm).map_err(Unplayable::Source)
    }

    /// [`Self::resolve`] for the work that reads a title unasked, a probe: where
    /// ffmpeg cannot read the relay, a title with no copy is warmed so the probe
    /// can run once it lands, and only a few titles are warmed at once.
    pub fn resolve_for_probe(&self, abs: &str) -> Result<Resolved, SourceError> {
        self.resolve_probing(abs, kroma_media::capabilities::network_input())
    }

    fn resolve_probing(&self, abs: &str, network: bool) -> Result<Resolved, SourceError> {
        if network || !remote_file::is_remote_file(Path::new(abs)) {
            return self.resolve(abs, false);
        }
        let found = self.find(abs, false)?;
        if matches!(found.place, Place::Upstream(_)) && self.memo.may_warm(abs) {
            self.find(abs, true)?;
        }
        self.input_of(abs, found, false)
    }

    fn input_of(&self, abs: &str, found: Found, network: bool) -> Result<Resolved, SourceError> {
        match found.place {
            Place::Copy(copy) => Ok(Resolved {
                input: MediaInput::Local(copy.path),
                stub: Some(found.stub),
            }),
            Place::Upstream(_) if network => {
                let relayed = self.relayed(abs, &found.stub)?;
                Ok(Resolved {
                    input: MediaInput::Remote(relayed),
                    stub: Some(found.stub),
                })
            }
            Place::Upstream(_) => Err(SourceError::Preparing),
        }
    }

    fn relayed(&self, abs: &str, stub: &RemoteFile) -> Result<Relayed, SourceError> {
        let (port, grant) = self.relay.grant(&self.me, abs).map_err(|error| {
            warn!(%error, "the loopback relay could not be opened");
            SourceError::Unreachable
        })?;
        let extension = stub.container().unwrap_or_default();
        Relayed::new(
            port,
            self.relay.secret(),
            &grant.id(),
            &extension,
            Arc::new(grant),
        )
        .ok_or(SourceError::Unreachable)
    }

    /// The file to read in full for the library file at `abs`, for work that
    /// must not stream a whole title: `abs` itself, or a stub's complete local
    /// copy. `None` while a stub's bytes are only a URL away. Never warms, and
    /// remembers a stub that has no copy yet for a few minutes and one whose
    /// source failed for a few seconds, so a pass over a library does not ask
    /// its source about every title again.
    pub fn local(&self, abs: &str) -> Result<Option<PathBuf>, SourceError> {
        if !remote_file::is_remote_file(Path::new(abs)) {
            return Ok(Some(PathBuf::from(abs)));
        }
        if let Some(Place::Copy(copy)) = self.cached(abs) {
            return Ok(Some(copy.path));
        }
        if let Some(known) = self.memo.recently(abs) {
            return known.map(|()| None);
        }
        match self.find(abs, false) {
            Ok(Found {
                place: Place::Copy(copy),
                ..
            }) => Ok(Some(copy.path)),
            Ok(_) => {
                self.memo.note_uncached(abs);
                Ok(None)
            }
            Err(error) => {
                self.memo.note_failure(abs, &error);
                Err(error)
            }
        }
    }

    /// [`Self::local`], and when a stub has no copy yet, a request to its source
    /// to start caching it (at most once per short window), so work that needs the
    /// whole file can run once the copy lands. Blocking.
    pub fn local_warming(&self, abs: &str) -> Result<Option<PathBuf>, SourceError> {
        let held = self.local(abs)?;
        if held.is_none() {
            self.find(abs, true)?;
        }
        Ok(held)
    }

    fn cached(&self, abs: &str) -> Option<Place> {
        let stub = remote_file::read(Path::new(abs)).ok()?;
        self.memo
            .recall(abs, &stub, false)
            .filter(|place| matches!(place, Place::Copy(_)))
    }

    /// Forgets what is remembered about `abs`, so its source is asked next time.
    pub fn forget(&self, abs: &str) {
        self.memo.forget(abs);
    }
}

/// Logs a failed resolution without anything a bearer link could be in.
pub fn log_failure(subject: &str, error: &SourceError) {
    warn!(%subject, %error, "a title's source could not be reached");
}
