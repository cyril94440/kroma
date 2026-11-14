use anyhow::Result;
use kroma_module_sdk::db;
use kroma_module_sdk::domain::{RequestKind, RequestStatus};
use kroma_module_sdk::host::{Event, HostStorage};
use kroma_module_sdk::primitives::now_ms;

use super::Rejection;
use crate::import::Placed;
use crate::peers::downloads::DownloadRow;
use crate::rules::blocklist::{self, Blocked};

/// The core read the file back and it was not what its release promised: the
/// release is blocklisted for this title, its files leave the library, and the
/// download is released.
pub fn reject<S: HostStorage>(state: &S, rejection: &Rejection) -> Result<bool> {
    let Some(kind) = RequestKind::parse(&rejection.kind) else {
        return Ok(false);
    };
    blocklist::add(
        state.store(),
        &Blocked {
            id: rejection.download_id.clone(),
            kind,
            tmdb_id: rejection.tmdb_id,
            release_title: rejection.release_title.clone(),
            info_hash: None,
            indexer_id: None,
            reason: format!("watch now: {}", rejection.reason),
            added_at: now_ms(),
        },
    )?;
    crate::import::remove_placed(&rejection.paths);
    reopen(state, rejection);
    release_download(state, &rejection.download_id, &rejection.reason);
    Ok(true)
}

pub(super) fn discard<S: HostStorage>(
    state: &S,
    download: &DownloadRow,
    reason: &str,
    placed: &[Placed],
) {
    let paths: Vec<String> = placed.iter().map(|p| p.path.clone()).collect();
    let rejection = Rejection {
        download_id: download.id.clone(),
        release_title: download.release_title.clone(),
        kind: crate::auto::title_kind(&download.kind).as_str().into(),
        tmdb_id: download.tmdb_id,
        reason: reason.into(),
        paths,
        request_id: download.request_id.clone(),
        season: download.season,
    };
    if let Err(e) = reject(state, &rejection) {
        tracing::warn!(target: "acquisition", download = %download.id, error = %format!("{e:#}"), "could not discard a release");
    }
}

fn release_download<S: HostStorage>(state: &S, id: &str, reason: &str) {
    let status = format!("watch now: {reason}");
    if let Err(e) = crate::peers::downloads::set_status(state, id, "removed", Some(&status)) {
        tracing::warn!(target: "acquisition", download = %id, error = %format!("{e:#}"), "could not release the download");
    }
    crate::peers::downloads::drop_data(state, id);
}

// The import marked what it covered available. With its files gone that is no
// longer so, and the availability pass flips back whatever the library still holds.
fn reopen<S: HostStorage>(state: &S, rejection: &Rejection) {
    let Some(request_id) = rejection.request_id.as_deref() else {
        return;
    };
    if let Err(e) = reopen_request(state, request_id, rejection.season) {
        tracing::warn!(target: "acquisition", request = %request_id, error = %format!("{e:#}"), "could not reopen the request");
    }
}

fn reopen_request<S: HostStorage>(state: &S, request_id: &str, season: Option<u32>) -> Result<()> {
    let conn = state.db().get()?;
    let rows = db::wanted_for_request(&conn, request_id)?;
    drop(conn);
    let reopened: Vec<String> = rows
        .iter()
        .filter(|row| matches!(row.status.as_str(), "available" | "grabbed"))
        .filter(|row| season.is_none() || row.season == season)
        .map(|row| row.id.clone())
        .collect();
    db::set_wanted_status(state.db(), &reopened, "wanted", now_ms())?;
    db::set_request_status(
        state.db(),
        request_id,
        RequestStatus::Approved,
        None,
        None,
        now_ms(),
    )?;
    state.publish(Event::new(
        "request.updated",
        serde_json::json!({ "id": request_id, "status": RequestStatus::Approved.as_str() }),
    ));
    Ok(())
}
