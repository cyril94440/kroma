use anyhow::Result;
use kroma_module_sdk::domain::RequestKind;
use kroma_module_sdk::host::HostStorage;
use kroma_module_sdk::primitives::now_ms;

use super::provisional::{self, Provisional};
use super::Rejection;
use crate::rules::blocklist::{self, Blocked};

/// The core read the file back and it was not what its release promised: the
/// release is blocklisted for this title, the files THIS import wrote leave the
/// library (through the recycle bin when one is set), the wanted rows the
/// download took go back, and the download is released. What is undone is what
/// was recorded for `download_id`: a path the caller names is never read, and a
/// download with no record undoes nothing.
pub fn reject<S: HostStorage>(state: &S, rejection: &Rejection) -> Result<bool> {
    let Some(held) = provisional::get(state.store(), &rejection.download_id)? else {
        return Ok(false);
    };
    blocklist_release(state, &held, &rejection.reason)?;
    undo(state, &held, &rejection.reason)?;
    Ok(true)
}

pub(super) fn blocklist_release<S: HostStorage>(
    state: &S,
    held: &Provisional,
    reason: &str,
) -> Result<()> {
    let Some(kind) = RequestKind::parse(&held.kind) else {
        return Ok(());
    };
    blocklist::add(
        state.store(),
        &Blocked {
            id: held.download_id.clone(),
            kind,
            tmdb_id: held.tmdb_id,
            release_title: held.release_title.clone(),
            info_hash: None,
            indexer_id: None,
            reason: format!("watch now: {reason}"),
            added_at: now_ms(),
        },
    )
}

/// Takes a provisional import back out without blaming its release: the files
/// it wrote, the wanted rows it took, the download.
pub(super) fn undo<S: HostStorage>(state: &S, held: &Provisional, reason: &str) -> Result<()> {
    crate::import::retire_written(state, &held.written);
    if let Some(request_id) = held.request_id.as_deref() {
        if let Err(e) = kroma_module_sdk::requests::lifecycle::release_wanted(
            state,
            request_id,
            &held.wanted_ids,
        ) {
            tracing::warn!(target: "acquisition", request = %request_id, error = %format!("{e:#}"), "could not reopen the request");
        }
    }
    release_download(state, &held.download_id, reason);
    provisional::forget(state.store(), &held.download_id)
}

fn release_download<S: HostStorage>(state: &S, id: &str, reason: &str) {
    let status = format!("watch now: {reason}");
    if let Err(e) = crate::peers::downloads::set_status(state, id, "removed", Some(&status)) {
        tracing::warn!(target: "acquisition", download = %id, error = %format!("{e:#}"), "could not release the download");
    }
    crate::peers::downloads::discard(state, id);
}
