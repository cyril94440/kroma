//! Watch now: a title nobody on this server holds, playing in seconds. The
//! acquisition module picks the best release the quality rules accept that the
//! download client holds already, and imports it. This service then catalogues
//! the file, reads it back, and only calls it ready when it is what its release
//! promised; a file that is not is thrown out and the next release is tried.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};

use crate::model::{Permission, RequestKind, User};
use crate::state::SharedState;

mod point;
mod verify;

pub use point::ACQUISITION;

use point::{Imported, Reply, Why};
use verify::{Facts, Promise, Verdict};

const TOTAL_BUDGET: Duration = Duration::from_secs(25);
const MODULE_BUDGET: Duration = Duration::from_secs(20);
const ATTEMPTS: usize = 3;

/// What one title or episode a viewer asked to watch now is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    pub kind: RequestKind,
    pub tmdb_id: u64,
    pub season: Option<u32>,
    pub episode: Option<u32>,
}

impl Scope {
    fn key(&self) -> String {
        format!(
            "{}:{}:{:?}:{:?}",
            self.kind.as_str(),
            self.tmdb_id,
            self.season,
            self.episode
        )
    }

    fn unit(&self) -> Option<(u32, u32)> {
        Some((self.season?, self.episode?))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    NoEngine,
    NoIndexer,
    NoRelease,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Ready {
        item_id: String,
        file_id: Option<String>,
    },
    Requested {
        request_id: Option<String>,
    },
    Unavailable {
        reason: Reason,
        request_id: Option<String>,
    },
}

/// Who may skip the queue: those whose request would have been fetched without
/// anyone's approval anyway.
pub fn may_watch_now(user: &User) -> bool {
    user.can(Permission::RequestsCreate)
        && (user.can(Permission::RequestsAuto) || user.can(Permission::RequestsManage))
}

/// One lock per title or episode, so two people asking for the same one at the
/// same moment import it once: the second waits, then finds it in the library.
#[derive(Default)]
pub struct Flights(Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>);

impl Flights {
    fn lock_for(&self, key: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut map = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        map.retain(|_, lock| Arc::strong_count(lock) > 1);
        map.entry(key.to_string()).or_default().clone()
    }

    #[cfg(test)]
    fn held(&self) -> usize {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).len()
    }
}

/// Whether this viewer should be offered the button: allowed to use it, and the
/// acquisition module live with a download client that can fetch at once.
/// Blocking.
pub fn offered(state: &SharedState, user: &User) -> bool {
    may_watch_now(user)
        && kroma_module_host::pinned_resolver(state.as_ref(), point::ACQUISITION, None)
            .is_some_and(|resolve| point::capable(&resolve))
}

pub async fn watch_now(state: &SharedState, user: &User, scope: &Scope) -> Result<Outcome> {
    let lock = state.watch_flights.lock_for(&scope.key());
    let _held = lock.lock().await;
    let (state, user, scope) = (state.clone(), user.clone(), scope.clone());
    tokio::task::spawn_blocking(move || Attempt::new(&state, &user, &scope).run()).await?
}

struct Attempt<'a> {
    state: &'a SharedState,
    user: &'a User,
    scope: &'a Scope,
    started: Instant,
    runtime_min: Option<Option<u32>>,
}

enum Read {
    Ready(String, Option<String>),
    Fake(String),
    Unverified,
}

impl<'a> Attempt<'a> {
    fn new(state: &'a SharedState, user: &'a User, scope: &'a Scope) -> Self {
        Self {
            state,
            user,
            scope,
            started: Instant::now(),
            runtime_min: None,
        }
    }

    fn run(mut self) -> Result<Outcome> {
        if let Some(item_id) = self.already_playable()? {
            return Ok(Outcome::Ready {
                item_id,
                file_id: None,
            });
        }
        let resolve =
            kroma_module_host::pinned_resolver(self.state.as_ref(), point::ACQUISITION, None)
                .ok_or_else(|| anyhow!("no module answers the acquisition point"))?;
        let mut turned_down: Vec<String> = Vec::new();
        let mut request_id = None;
        for _ in 0..ATTEMPTS {
            let left = TOTAL_BUDGET.saturating_sub(self.started.elapsed());
            if left.is_zero() {
                break;
            }
            let reply = point::ask(&resolve, &self.ask(&turned_down, left.min(MODULE_BUDGET)))?;
            let imported = match reply {
                Reply::Requested { request_id } => {
                    return Ok(Outcome::Requested {
                        request_id: Some(request_id),
                    })
                }
                Reply::Unavailable { reason, request_id } => {
                    return Ok(Outcome::Unavailable {
                        reason: reason_of(reason),
                        request_id,
                    });
                }
                Reply::Imported(imported) => imported,
            };
            request_id = Some(imported.request_id.clone());
            match self.read_back(&imported)? {
                Read::Ready(item_id, file_id) => {
                    self.probe_the_rest();
                    return Ok(Outcome::Ready { item_id, file_id });
                }
                Read::Unverified => break,
                Read::Fake(reason) => {
                    self.turn_down(&resolve, &imported, &reason);
                    turned_down.push(imported.release.key);
                }
            }
        }
        Ok(Outcome::Requested { request_id })
    }

    fn ask<'b>(&'b self, exclude: &'b [String], budget: Duration) -> point::Ask<'b> {
        point::Ask {
            kind: self.scope.kind.as_str(),
            tmdb_id: self.scope.tmdb_id,
            season: self.scope.season,
            episode: self.scope.episode,
            user_id: &self.user.id,
            exclude,
            budget_ms: budget.as_millis() as u64,
        }
    }

    fn already_playable(&self) -> Result<Option<String>> {
        let conn = self.state.db.get()?;
        Ok(
            crate::db::playable_items(&conn, self.scope.tmdb_id, self.scope.unit())?
                .into_iter()
                .find(|(_, library)| self.user.sees_library(library))
                .map(|(id, _)| id),
        )
    }

    fn read_back(&mut self, imported: &Imported) -> Result<Read> {
        let paths: Vec<PathBuf> = imported.paths.iter().map(PathBuf::from).collect();
        let found = crate::services::scan::ingest_files(self.state, &paths)?;
        let primary = canonical(&imported.primary_path);
        let Some(file) = found.iter().find(|f| f.abs_path == primary) else {
            return Err(anyhow!("the imported file is not in the catalogue"));
        };
        let probed = crate::infra::probe::probe_one(
            &self.state.db,
            self.state.ffprobe_available,
            &self.state.sources,
            &self.state.events,
            &file.file_id,
            &file.abs_path,
            &file.item_id,
        );
        if let Err(error) = probed {
            tracing::warn!(error = %format!("{error:#}"), "watch now: could not probe the imported file");
            return Ok(Read::Unverified);
        }
        let Some(item) = crate::db::get_item(&self.state.db, &file.item_id)? else {
            return Ok(Read::Unverified);
        };
        let Some(stored) = item.files.iter().find(|f| f.id == file.file_id) else {
            return Ok(Read::Unverified);
        };
        if !stored.probed && stored.unreadable.is_none() {
            return Ok(Read::Unverified);
        }
        let facts = Facts {
            unreadable: stored.unreadable.clone(),
            duration_ms: stored.duration_ms,
            width: stored.video.as_ref().and_then(|v| v.width),
            height: stored.video.as_ref().and_then(|v| v.height),
        };
        let promise = Promise {
            runtime_min: self.runtime(),
            claimed_height: imported.release.claimed_height,
        };
        match verify::judge(&facts, &promise) {
            Verdict::Fake(reason) => Ok(Read::Fake(reason)),
            Verdict::Genuine if !self.user.sees_library(&item.library) => Ok(Read::Unverified),
            Verdict::Genuine => Ok(Read::Ready(item.id, Some(file.file_id.clone()))),
        }
    }

    fn turn_down(&self, resolve: &kroma_module_host::Resolver, imported: &Imported, reason: &str) {
        let rejection = point::Rejection {
            download_id: &imported.download_id,
            release_title: &imported.release.title,
            kind: self.scope.kind.as_str(),
            tmdb_id: self.scope.tmdb_id,
            reason,
            paths: &imported.paths,
            request_id: &imported.request_id,
            season: self.scope.season,
        };
        if let Err(error) = point::reject(resolve, &rejection) {
            tracing::warn!(error = %format!("{error:#}"), "watch now: the module could not discard a release");
        }
        let gone: Vec<String> = imported.paths.iter().map(|p| canonical(p)).collect();
        if let Err(error) = crate::services::scan::forget_files(self.state, &gone) {
            tracing::warn!(error = %format!("{error:#}"), "watch now: could not forget the files of a rejected release");
        }
    }

    fn probe_the_rest(&self) {
        crate::infra::probe::spawn_probe_pass(
            self.state.db.clone(),
            self.state.ffprobe_available,
            self.state.events.clone(),
            self.state.activity.clone(),
            self.state.sources.clone(),
        );
    }

    fn runtime(&mut self) -> Option<u32> {
        *self
            .runtime_min
            .get_or_insert_with(|| tmdb_runtime(self.state, self.scope))
    }
}

fn reason_of(why: Why) -> Reason {
    match why {
        Why::NoEngine | Why::Other => Reason::NoEngine,
        Why::NoIndexer => Reason::NoIndexer,
        Why::NoRelease => Reason::NoRelease,
    }
}

fn canonical(path: &str) -> String {
    Path::new(path)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(path))
        .to_string_lossy()
        .into_owned()
}

fn tmdb_runtime(state: &SharedState, scope: &Scope) -> Option<u32> {
    use crate::infra::metadata::{discover, season_episodes};
    let key = state.config.tmdb_api_key.as_deref()?;
    let language = crate::services::settings::metadata_language(&state.settings, &state.config);
    match scope.unit() {
        None => {
            discover::detail(key, &language, scope.kind, scope.tmdb_id)
                .ok()??
                .runtime_min
        }
        Some((season, episode)) => {
            season_episodes(key, &language, scope.tmdb_id, season)
                .episodes
                .into_iter()
                .find(|e| e.episode == episode)?
                .runtime_min
        }
    }
}

#[cfg(test)]
mod tests;
