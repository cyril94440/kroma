use anyhow::Result;
use kroma_module_sdk::host::HostStorage;
use kroma_module_sdk::primitives::now_ms;

use super::provisional::{self, Provisional, GRACE_MS};
use super::Confirmation;

/// The core read the file back and it is what its release promised: the
/// download is recorded as imported, the request becomes available and its
/// people are told. `false` when nothing is awaiting a verdict for the download.
pub fn confirm<S: HostStorage>(state: &S, confirmation: &Confirmation) -> Result<bool> {
    let Some(held) = provisional::get(state.store(), &confirmation.download_id)? else {
        return Ok(false);
    };
    finalize(state, &held)?;
    Ok(true)
}

/// A provisional import the core never answered for, because it died or timed
/// out, is finalized once [`GRACE_MS`] has passed. Nothing judged it a fake, and
/// the regular import would have taken the same file unread; taking it out
/// instead could unlink something a viewer is already playing.
pub fn reconcile<S: HostStorage>(state: &S, log: &dyn Fn(String)) -> Result<usize> {
    let stale = provisional::older_than(state.store(), now_ms() - GRACE_MS)?;
    let mut finalized = 0;
    for held in stale {
        match finalize(state, &held) {
            Ok(()) => {
                log(format!(
                    "watch now: no verdict for \"{}\"; imported as it was",
                    held.release_title
                ));
                finalized += 1;
            }
            Err(e) => log(format!(
                "watch now: could not finalize \"{}\": {e:#}",
                held.release_title
            )),
        }
    }
    Ok(finalized)
}

fn finalize<S: HostStorage>(state: &S, held: &Provisional) -> Result<()> {
    let waiting = crate::peers::downloads::completed(state)?
        .into_iter()
        .find(|row| row.id == held.download_id);
    match waiting {
        Some(row) => crate::import::finalize_placed(state, &row, &held.paths)?,
        None if crate::import::recorded_paths(state, &held.download_id)?.is_some() => {}
        None => tracing::warn!(
            target: "acquisition",
            download = %held.download_id,
            "a watch-now import has no download left to record against"
        ),
    }
    provisional::forget(state.store(), &held.download_id)?;
    state.trigger_job("library.scan", "acquisition-import");
    Ok(())
}
