//! Re-reading files an older probe described, gradually and behind everything else.
//!
//! A release that makes the probe learn something new (HDR format, Dolby Vision
//! detail, an audio profile) leaves every file that could gain from it marked
//! stale. They stay probed: every reader keeps the facts it has, and the new ones
//! replace them in one write once a file has been read again. A library of
//! thousands is not read again in one go, so a pass takes a bounded batch, one
//! file at a time, each under a slot of the ffmpeg budget that a viewer's
//! re-encode takes away.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tracing::{debug, info, warn};

use crate::db::{self, FileProbe, Pool};
use crate::infra::events::{Bus, ServerEvent};
use crate::services::source::{SourceError, Sources};

use super::{probe_outcome, Deferred, Probed};

/// Files a pass takes. Each read is a short ffprobe, so a batch is a couple of
/// minutes of one process; the next boot or scan takes the next one, which
/// refreshes a library of ten thousand over a week of ordinary use without ever
/// competing with the first scan of a new one.
const BATCH: usize = 200;
const PAUSE: Duration = Duration::from_millis(250);

static RUNNING: AtomicBool = AtomicBool::new(false);

struct Running;

impl Running {
    fn claim() -> Option<Self> {
        (!RUNNING.swap(true, Ordering::AcqRel)).then_some(Self)
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        RUNNING.store(false, Ordering::Release);
    }
}

/// [`refresh_stale`] unless another pass is already doing it: a boot and the scan
/// that follows it both ask, and one reader of a library at a time is enough.
pub(super) fn refresh_once(pool: &Pool, ffprobe_present: bool, sources: &Sources, bus: &Bus) {
    if let Some(_running) = Running::claim() {
        refresh_stale(pool, ffprobe_present, sources, bus);
    }
}

fn refresh_stale(pool: &Pool, ffprobe_present: bool, sources: &Sources, bus: &Bus) {
    if !ffprobe_present {
        return;
    }
    let batch = match db::stale_probe_files(pool, BATCH) {
        Ok(batch) => batch,
        Err(error) => {
            warn!(%error, "failed to list the files an older probe described");
            return;
        }
    };
    if batch.is_empty() {
        return;
    }
    info!(
        files = batch.len(),
        left = db::stale_probe_count(pool).unwrap_or(batch.len()),
        "re-reading files an older probe described"
    );
    let mut refreshed = 0;
    for (file_id, abs_path, _item_id) in batch {
        match reread(pool, sources, &file_id, &abs_path) {
            Ok(true) => refreshed += 1,
            Ok(false) => {}
            Err(error) if error.downcast_ref::<Deferred>().is_some() => {
                debug!(file = %file_id, %error, "re-read deferred");
            }
            Err(error) => warn!(file = %file_id, %error, "failed to re-read; it is offered again"),
        }
        std::thread::sleep(PAUSE);
    }
    if refreshed > 0 {
        bus.publish(ServerEvent::LibraryUpdated);
    }
}

fn reread(pool: &Pool, sources: &Sources, file_id: &str, abs_path: &str) -> anyhow::Result<bool> {
    let resolved = match sources.resolve_without_warming(abs_path) {
        Ok(resolved) => resolved,
        Err(SourceError::Preparing) => return Err(Deferred::Preparing.into()),
        Err(error) if error.is_transient() => return Err(error.into()),
        Err(_) => {
            db::keep_probe_facts(pool, file_id)?;
            return Ok(false);
        }
    };
    let container = resolved.container().unwrap_or_default();
    let _slot = kroma_media::ffmpeg_gate::acquire();
    match probe_outcome(&resolved.input, &container, true) {
        Probed::Done(result) if result.unreadable.is_some() => {
            db::keep_probe_facts(pool, file_id)?;
            Ok(false)
        }
        Probed::Done(result) => {
            db::set_file_probe(
                pool,
                file_id,
                &FileProbe {
                    duration_ms: result.duration_ms,
                    video: result.video.as_ref(),
                    audio: result.audio.as_ref(),
                    audio_tracks: &result.audio_tracks,
                    subtitles: &result.subtitles,
                    unreadable: None,
                },
            )?;
            Ok(true)
        }
        Probed::Killed => Ok(false),
        Probed::Network => Err(Deferred::Network.into()),
    }
}

#[cfg(test)]
mod tests;
