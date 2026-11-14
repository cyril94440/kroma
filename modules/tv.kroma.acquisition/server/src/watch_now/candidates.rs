use anyhow::Result;
use kroma_module_sdk::host::HostStorage;
use kroma_module_sdk::primitives::now_ms;

use crate::peers::downloads::InstantRelease;
use crate::search::remembered::{recall_all, RememberedRelease};
use crate::search::subject::{resolve, Subject};
use crate::search::{grab_spec_from_release, interactive_search, wanted_ids_for, SearchScope};

pub(super) const MAX_CANDIDATES: usize = 8;
const FRESH_FOR_MS: i64 = 120_000;

pub(super) fn key_of(release: &RememberedRelease) -> String {
    format!("{}|{}", release.view.guid, release.view.indexer_id)
}

/// The releases the title's own rules accept, best first, that no one has
/// turned down: the same judgement the automatic search applies, never widened.
/// A pack search keeps only packs, because the episode's own releases were asked
/// about already.
pub(super) fn ranked<S: HostStorage>(
    state: &S,
    request_id: &str,
    scope: SearchScope,
    skip: &[String],
) -> Result<Vec<InstantRelease>> {
    let subject = Subject::request(request_id);
    let shown = shown_by_a_recent_search(state, &subject, scope, skip)?;
    let wanted_target = matches!(scope, SearchScope::Season { .. }).then_some("season");
    let mut accepted: Vec<RememberedRelease> = shown
        .into_iter()
        .filter(|r| r.view.grabbable && r.view.rejected.is_none() && r.view.score.is_some())
        .filter(|r| !r.view.upgrade)
        .filter(|r| wanted_target.is_none_or(|target| r.view.target == target))
        .filter(|r| !skip.contains(&key_of(r)))
        .collect();
    accepted.truncate(MAX_CANDIDATES);
    let resolved = resolve(state, &subject, scope)?;
    let mut offered = Vec::new();
    for release in accepted {
        let Some(link) = link_of(state, &release) else {
            continue;
        };
        let wanted_ids = wanted_ids_for(&resolved.ledger, &release.view);
        offered.push(InstantRelease {
            key: key_of(&release),
            spec: grab_spec_from_release(
                &release.view,
                &link,
                release.tmdb_id,
                Some(resolved.title.clone()),
                resolved.year,
                resolved.request_id.clone(),
                wanted_ids,
                false,
            ),
        });
    }
    Ok(offered)
}

fn shown_by_a_recent_search<S: HostStorage>(
    state: &S,
    subject: &Subject,
    scope: SearchScope,
    skip: &[String],
) -> Result<Vec<RememberedRelease>> {
    if !skip.is_empty() {
        let kept = recall_all(state.store(), &subject.cache_key(scope))?;
        if let Some((releases, _)) = kept.filter(|(_, at)| now_ms() - at < FRESH_FOR_MS) {
            return Ok(releases);
        }
    }
    interactive_search(state, subject, scope)?;
    Ok(recall_all(state.store(), &subject.cache_key(scope))?
        .map(|(releases, _)| releases)
        .unwrap_or_default())
}

fn link_of<S: HostStorage>(state: &S, release: &RememberedRelease) -> Option<String> {
    match crate::search::interactive::resolve_grab_target(state, release) {
        Ok(link) if !link.is_empty() => Some(link),
        Ok(_) => None,
        Err(e) => {
            tracing::debug!(release = %release.view.title, error = %format!("{e:#}"), "no link for a candidate");
            None
        }
    }
}
