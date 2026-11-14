//! Watch now: the fastest honest route from a title nobody holds to a file the
//! library can play. It asks the same questions the automatic search asks, of
//! the same rules, and only then asks the download client which of the accepted
//! releases it holds already.
//!
//! The import is provisional: files are placed and recorded, and nothing is
//! announced until the core, having read the file back, answers
//! `watch-now-confirm`; `watch-now-reject` takes back exactly what was placed.

use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use kroma_module_sdk::domain::{CreateRequestBody, EpisodeRef, RequestKind};
use kroma_module_sdk::host::HostStorage;
use kroma_module_sdk::primitives::now_ms;

use crate::peers::downloads::{grab_instant, InstantRelease, InstantStatus};
use crate::search::SearchScope;
use crate::rules::blocklist::{self, Blocked};

mod candidates;
mod confirm;
mod placing;
mod present;
mod provisional;
mod reject;
mod wire;

pub use confirm::{confirm, reconcile};
pub use provisional::is_held_for_watch_now;
pub use reject::{flag, reject};
pub use wire::{Ask, Confirmation, Flag, Imported, Rejection, Release, Reply, Why};

use placing::{place, place_ready, Placing};
use provisional::Intents;

const DEFAULT_BUDGET_MS: u64 = 20_000;
const TRIES_PER_SCOPE: usize = 3;
const IMPORT_RESERVE: Duration = Duration::from_secs(2);
const MIN_GRAB: Duration = Duration::from_secs(1);
const SEARCH_SHARE: f64 = 0.55;

/// Whether this server could fetch a title at once: the preferred download
/// client answers instant, and an indexer is there to ask.
pub fn capable<S: HostStorage>(state: &S) -> bool {
    crate::peers::downloads::instant_capable(state)
        && crate::peers::indexers::enabled(state).is_ok_and(|found| !found.is_empty())
}

pub fn watch_now<S: HostStorage>(state: &S, ask: &Ask) -> Result<Reply> {
    let started = Instant::now();
    let budget = Duration::from_millis(ask.budget_ms.unwrap_or(DEFAULT_BUDGET_MS));
    let kind = RequestKind::parse(&ask.kind).ok_or_else(|| anyhow!("unknown kind {}", ask.kind))?;
    if !crate::peers::downloads::instant_capable(state) {
        return Ok(unavailable(Why::NoEngine, None));
    }
    if !crate::peers::indexers::enabled(state).is_ok_and(|found| !found.is_empty()) {
        return Ok(unavailable(Why::NoIndexer, None));
    }
    if !user_may_skip_the_queue(state, &ask.user_id)? {
        return Err(anyhow!("this user may not watch now"));
    }
    let unit = asked_unit(kind, ask)?;
    if present::in_library(state, kind, ask.tmdb_id, unit)? {
        return Ok(Reply::Present { request_id: None });
    }
    let request = kroma_module_sdk::requests::lifecycle::create_approved(
        state,
        &ask.user_id,
        &CreateRequestBody {
            kind,
            tmdb_id: ask.tmdb_id,
            seasons: None,
            episodes: unit.map(|(season, episode)| vec![EpisodeRef { season, episode }]),
        },
    )?;
    match present::on_its_way(state, kind, ask.tmdb_id, unit)? {
        present::OnItsWay::Underway => return Ok(Reply::Requested { request_id: request.id }),
        present::OnItsWay::Ready(id) => {
            if let Some(reply) = place_ready(state, &request.id, &id, unit, &id)? {
                return Ok(reply);
            }
        }
        present::OnItsWay::Nothing => {}
    }
    let mut skip = ask.exclude.clone();
    let mut saw_a_candidate = false;
    let mut intents = Intents::new(state.store());
    for scope in scopes(unit) {
        for _ in 0..TRIES_PER_SCOPE {
            if time_left(started, budget) < MIN_GRAB {
                return Ok(Reply::Requested { request_id: request.id });
            }
            let search_by = started + budget.mul_f64(SEARCH_SHARE);
            let offered = crate::peers::deadline::within(search_by.max(Instant::now()), || {
                candidates::ranked(state, &request.id, scope, &skip)
            })?;
            if offered.is_empty() {
                break;
            }
            saw_a_candidate = true;
            intents.note(offered.iter().map(|r| r.spec.magnet_or_url.trim().to_string()));
            let grab_for = time_left(started, budget);
            if grab_for < MIN_GRAB {
                return Ok(Reply::Requested { request_id: request.id });
            }
            let outcome = grab_instant(state, &offered, grab_for)?;
            match outcome.status {
                InstantStatus::Grabbed => {}
                InstantStatus::Unsupported => {
                    return Ok(unavailable(Why::NoEngine, Some(request.id)));
                }
                InstantStatus::Nothing | InstantStatus::Unknown => break,
                InstantStatus::Failed => {
                    if let Some(key) = outcome.key {
                        if outcome.deterministic {
                            blocklist_failed(state, &offered, &key, kind, ask.tmdb_id, outcome.reason.as_deref());
                        }
                        skip.push(key);
                    }
                    continue;
                }
            }
            let (Some(key), Some(download)) = (outcome.key, outcome.download) else {
                break;
            };
            let wanted_ids = offered
                .iter()
                .find(|r| r.key == key)
                .map(|r| r.spec.wanted_ids.clone())
                .unwrap_or_default();
            match place(state, &request.id, download, &key, unit, wanted_ids)? {
                Placing::Reply(reply) => return Ok(reply),
                Placing::Rejected => skip.push(key),
            }
        }
    }
    tracing::info!(target: "acquisition", "watch now: nothing instant for title {}", ask.tmdb_id);
    Ok(if saw_a_candidate {
        Reply::Requested { request_id: request.id }
    } else {
        unavailable(Why::NoRelease, Some(request.id))
    })
}

fn time_left(started: Instant, budget: Duration) -> Duration {
    budget
        .saturating_sub(started.elapsed())
        .saturating_sub(IMPORT_RESERVE)
}

fn blocklist_failed<S: HostStorage>(
    state: &S,
    offered: &[InstantRelease],
    key: &str,
    kind: RequestKind,
    tmdb_id: u64,
    reason: Option<&str>,
) {
    let Some(release) = offered.iter().find(|r| r.key == key) else {
        return;
    };
    let entry = Blocked {
        id: format!("watch-now:{tmdb_id}:{key}"),
        kind,
        tmdb_id,
        release_title: release.spec.release_title.clone(),
        info_hash: None,
        indexer_id: None,
        reason: format!("watch now: {}", reason.unwrap_or("the engine could not use it")),
        added_at: now_ms(),
    };
    if let Err(e) = blocklist::add(state.store(), &entry) {
        tracing::warn!(target: "acquisition", error = %format!("{e:#}"), "could not blocklist an unusable release");
    }
}

fn unavailable(reason: Why, request_id: Option<String>) -> Reply {
    Reply::Unavailable { reason, request_id }
}

fn user_may_skip_the_queue<S: HostStorage>(state: &S, user_id: &str) -> Result<bool> {
    let conn = state.db().get()?;
    let stored: Option<String> = conn
        .query_row(
            "SELECT permissions FROM users WHERE id = ?1",
            [user_id],
            |r| r.get(0),
        )
        .ok();
    let held: Vec<String> = stored
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    let holds = |name: &str| held.iter().any(|each| each == name);
    Ok(holds("requests.create") && (holds("requests.auto") || holds("requests.manage")))
}

fn asked_unit(kind: RequestKind, ask: &Ask) -> Result<Option<(u32, u32)>> {
    match (kind, ask.season, ask.episode) {
        (RequestKind::Movie, ..) => Ok(None),
        (RequestKind::Show, Some(season), Some(episode)) => Ok(Some((season, episode))),
        (RequestKind::Show, ..) => Err(anyhow!("a show is watched one episode at a time")),
    }
}

fn scopes(unit: Option<(u32, u32)>) -> Vec<SearchScope> {
    match unit {
        None => vec![SearchScope::Movie],
        Some((season, episode)) => {
            vec![
                SearchScope::Episode { season, episode },
                SearchScope::Season { season },
            ]
        }
    }
}

#[cfg(test)]
mod test_requests;
#[cfg(test)]
mod test_world;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_verdict;
#[cfg(test)]
mod tests_wire;
