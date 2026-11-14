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

use crate::import::{Landing, Placed};
use crate::peers::downloads::{grab_instant, DownloadRow, InstantRelease, InstantStatus};
use crate::search::SearchScope;
use crate::rules::blocklist::{self, Blocked};

mod candidates;
mod confirm;
mod present;
mod provisional;
mod reject;
mod wire;

pub use confirm::{confirm, reconcile};
pub use provisional::is_held_for_watch_now;
pub use reject::reject;
pub use wire::{Ask, Confirmation, Imported, Rejection, Release, Reply, Why};

use provisional::{Intents, Provisional};

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

enum Placing {
    Reply(Reply),
    Rejected,
}

fn place_ready<S: HostStorage>(
    state: &S,
    request_id: &str,
    download_id: &str,
    unit: Option<(u32, u32)>,
    key: &str,
) -> Result<Option<Reply>> {
    let Some(row) = crate::peers::downloads::completed(state)?
        .into_iter()
        .find(|row| row.id == download_id)
    else {
        return Ok(None);
    };
    Ok(match place(state, request_id, row, key, unit, Vec::new())? {
        Placing::Reply(reply) => Some(reply),
        Placing::Rejected => None,
    })
}

fn place<S: HostStorage>(
    state: &S,
    request_id: &str,
    download: DownloadRow,
    key: &str,
    unit: Option<(u32, u32)>,
    wanted_ids: Vec<String>,
) -> Result<Placing> {
    let Some((row, landing)) = crate::import::place_download(state, &download.id)? else {
        return Ok(already_taken(state, request_id, &download, key, unit)?);
    };
    let held = Provisional {
        download_id: row.id.clone(),
        request_id: Some(request_id.to_string()),
        kind: crate::auto::title_kind(&row.kind).as_str().into(),
        tmdb_id: row.tmdb_id,
        season: row.season,
        release_title: row.release_title.clone(),
        paths: landing.videos.iter().map(|p| p.path.clone()).collect(),
        written: landing.written.clone(),
        wanted_ids,
        created_at: now_ms(),
    };
    provisional::save(state.store(), &held)?;
    let Some(primary) = primary_of(&landing.videos, unit) else {
        let reason = "the pack does not hold the episode asked for";
        reject::blocklist_release(state, &held, reason)?;
        reject::undo(state, &held, reason)?;
        return Ok(Placing::Rejected);
    };
    if !primary.created {
        reject::undo(state, &held, "already in the library")?;
        return Ok(Placing::Reply(Reply::Present {
            request_id: Some(request_id.to_string()),
        }));
    }
    Ok(Placing::Reply(Reply::Imported(Imported {
        request_id: request_id.to_string(),
        download_id: row.id,
        release: release_of(key, &row.release_title),
        paths: created_paths(&landing),
        primary_path: primary.path.clone(),
    })))
}

fn already_taken<S: HostStorage>(
    state: &S,
    request_id: &str,
    download: &DownloadRow,
    key: &str,
    unit: Option<(u32, u32)>,
) -> Result<Placing> {
    let Some(paths) = crate::import::recorded_paths(state, &download.id)? else {
        return Ok(Placing::Reply(Reply::Requested {
            request_id: request_id.to_string(),
        }));
    };
    let primary = paths
        .iter()
        .find(|path| unit.is_none() || unit_of_name(path) == unit)
        .or_else(|| paths.first());
    Ok(match primary {
        Some(primary) => Placing::Reply(Reply::Imported(Imported {
            request_id: request_id.to_string(),
            download_id: download.id.clone(),
            release: release_of(key, &download.release_title),
            primary_path: primary.clone(),
            paths,
        })),
        None => Placing::Reply(Reply::Requested {
            request_id: request_id.to_string(),
        }),
    })
}

fn release_of(key: &str, title: &str) -> Release {
    Release {
        key: key.to_string(),
        title: title.to_string(),
        claimed_height: kroma_scene::parse_release_name(title)
            .resolution
            .map(height_of),
    }
}

fn unit_of_name(path: &str) -> Option<(u32, u32)> {
    let stem = std::path::Path::new(path).file_stem()?.to_str()?;
    let parsed = kroma_scene::parse_release_name(stem);
    Some((parsed.season?, parsed.episode?))
}

fn created_paths(landing: &Landing) -> Vec<String> {
    landing
        .videos
        .iter()
        .filter(|p| p.created)
        .map(|p| p.path.clone())
        .collect()
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

fn primary_of(placed: &[Placed], unit: Option<(u32, u32)>) -> Option<&Placed> {
    match unit {
        None => placed.first(),
        Some(wanted) => placed.iter().find(|p| p.unit == Some(wanted)),
    }
}

fn height_of(resolution: kroma_scene::Res) -> u32 {
    match resolution {
        kroma_scene::Res::Sd => 480,
        kroma_scene::Res::R720 => 720,
        kroma_scene::Res::R1080 => 1080,
        kroma_scene::Res::R2160 => 2160,
    }
}

#[cfg(test)]
mod test_world;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_verdict;
