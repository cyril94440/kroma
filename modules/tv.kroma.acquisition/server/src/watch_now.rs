//! Watch now: the fastest honest route from a title nobody holds to a file the
//! library can play. It asks the same questions the automatic search asks, of
//! the same rules, and only then asks the download client which of the accepted
//! releases it holds already.

use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use kroma_module_sdk::domain::{CreateRequestBody, EpisodeRef, RequestKind};
use kroma_module_sdk::host::HostStorage;

use crate::import::Placed;
use crate::peers::downloads::{grab_instant, InstantStatus};
use crate::search::SearchScope;

mod candidates;
mod reject;
mod wire;

pub use reject::reject;
pub use wire::{Ask, Imported, Rejection, Release, Reply, Why};

const DEFAULT_BUDGET_MS: u64 = 20_000;
const TRIES_PER_SCOPE: usize = 2;

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
        return Ok(Reply::Unavailable {
            reason: Why::NoEngine,
            request_id: None,
        });
    }
    if !crate::peers::indexers::enabled(state).is_ok_and(|found| !found.is_empty()) {
        return Ok(Reply::Unavailable {
            reason: Why::NoIndexer,
            request_id: None,
        });
    }
    let unit = asked_unit(kind, ask)?;
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
    let mut skip = ask.exclude.clone();
    let mut saw_a_candidate = false;
    for scope in scopes(unit) {
        for _ in 0..TRIES_PER_SCOPE {
            if started.elapsed() >= budget {
                return Ok(Reply::Requested {
                    request_id: request.id,
                });
            }
            let offered = candidates::ranked(state, &request.id, scope, &skip)?;
            if offered.is_empty() {
                break;
            }
            saw_a_candidate = true;
            let outcome = grab_instant(state, &offered)?;
            match outcome.status {
                InstantStatus::Grabbed => {}
                InstantStatus::Unsupported => {
                    return Ok(Reply::Unavailable {
                        reason: Why::NoEngine,
                        request_id: Some(request.id),
                    });
                }
                InstantStatus::Nothing | InstantStatus::Unknown => break,
            }
            let (Some(key), Some(download)) = (outcome.key, outcome.download) else {
                break;
            };
            let placed = crate::import::import_download(state, &download.id)?;
            let Some(primary) = primary_of(&placed, unit) else {
                reject::discard(
                    state,
                    &download,
                    "the pack does not hold the episode asked for",
                    &placed,
                );
                skip.push(key);
                continue;
            };
            let claimed_height = kroma_scene::parse_release_name(&download.release_title)
                .resolution
                .map(height_of);
            return Ok(Reply::Imported(Imported {
                request_id: request.id,
                download_id: download.id,
                release: Release {
                    key,
                    title: download.release_title,
                    claimed_height,
                },
                paths: placed.iter().map(|p| p.path.clone()).collect(),
                primary_path: primary,
            }));
        }
    }
    tracing::info!(target: "acquisition", "watch now: nothing instant for title {}", ask.tmdb_id);
    Ok(if saw_a_candidate {
        Reply::Requested {
            request_id: request.id,
        }
    } else {
        Reply::Unavailable {
            reason: Why::NoRelease,
            request_id: Some(request.id),
        }
    })
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

fn primary_of(placed: &[Placed], unit: Option<(u32, u32)>) -> Option<String> {
    match unit {
        None => placed.first().map(|p| p.path.clone()),
        Some(wanted) => placed
            .iter()
            .find(|p| p.unit == Some(wanted))
            .map(|p| p.path.clone()),
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
mod tests;
