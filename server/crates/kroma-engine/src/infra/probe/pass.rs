//! Probing a whole library: the phase-2 background pass, its worker pool, and
//! the single-file write both it and the `pipeline.probe` stage go through.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use tracing::{info, warn};

use crate::db::{self, FileProbe, Pool};
use crate::infra::events::{Bus, ServerEvent};
use crate::services::activity::{self, Shared as Activity};

use crate::services::source::Sources;

use super::{probe_outcome, Probed};

// Half the cores, clamped to 2..4: each ffprobe is a real process, and more at
// once starves interactive work on a small NAS. `KROMA_PROBE_WORKERS` overrides.
fn probe_workers() -> usize {
    if let Some(n) = std::env::var("KROMA_PROBE_WORKERS")
        .ok()
        .and_then(|s| s.parse().ok())
    {
        return n;
    }
    let cores = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(4);
    (cores / 2).clamp(2, 4)
}

/// Why a probe stopped without anything being wrong with the file, found in the
/// error `probe_one` returns (`error.downcast_ref::<Deferred>()`). Nothing was
/// recorded: the file stays unprobed and a later pass reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deferred {
    /// The title is not on this machine yet and this ffmpeg cannot read one over
    /// the network: its source was asked to cache it (at most a few at a time),
    /// and the probe runs once the copy lands.
    Preparing,
    /// The title's bytes could not be fetched this time.
    Network,
}

impl std::fmt::Display for Deferred {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Preparing => "the title is still being cached and cannot be probed yet",
            Self::Network => "the title could not be fetched to be probed",
        })
    }
}

impl std::error::Error for Deferred {}

struct ProbeJob {
    file_id: String,
    abs_path: String,
    item_id: String,
}

/// ffprobe every file with `probed=0`, write the result, and emit live events so
/// clients fill in codec/HDR badges. Returns immediately; work runs on a small
/// pool of detached threads. A no-op when there are no unprobed files.
pub fn spawn_probe_pass(
    pool: Pool,
    ffprobe_present: bool,
    bus: Bus,
    activity: Activity,
    sources: Arc<Sources>,
) {
    let unprobed = match db::unprobed_files(&pool) {
        Ok(v) => v,
        Err(e) => {
            warn!(error = %e, "failed to list unprobed files; skipping probe pass");
            return;
        }
    };
    if unprobed.is_empty() {
        info!("phase-2 probe: nothing to probe (mtime cache hit)");
        return;
    }

    let total = unprobed.len();
    info!(files = total, "starting phase-2 background probing");
    activity::probe_started(&activity, total);

    let jobs: Vec<ProbeJob> = unprobed
        .into_iter()
        .map(|(file_id, abs_path, item_id)| ProbeJob {
            file_id,
            abs_path,
            item_id,
        })
        .collect();
    let queue = Arc::new(Mutex::new(jobs));
    let done = Arc::new(AtomicUsize::new(0));

    thread::spawn(move || {
        run_probe_pass(pool, ffprobe_present, bus, activity, sources, queue, done, total)
    });
}

#[allow(clippy::too_many_arguments)]
fn run_probe_pass(
    pool: Pool,
    ffprobe_present: bool,
    bus: Bus,
    activity: Activity,
    sources: Arc<Sources>,
    queue: Arc<Mutex<Vec<ProbeJob>>>,
    done: Arc<AtomicUsize>,
    total: usize,
) {
    let worker_count = probe_workers().min(total.max(1));
    let mut handles = Vec::with_capacity(worker_count);
    for _ in 0..worker_count {
        let pool = pool.clone();
        let queue = queue.clone();
        let done = done.clone();
        let bus = bus.clone();
        let activity = activity.clone();
        let sources = sources.clone();
        handles.push(thread::spawn(move || {
            probe_worker_loop(
                &pool,
                ffprobe_present,
                &sources,
                &queue,
                &done,
                &bus,
                &activity,
                total,
            )
        }));
    }
    for h in handles {
        let _ = h.join();
    }
    let done = done.load(Ordering::Relaxed);
    activity::probe_completed(&activity);
    info!(probed = done, total, "phase-2 probing complete");
    bus.publish(ServerEvent::ProbeProgress { done, total });
    bus.publish(ServerEvent::ProbeCompleted { total });
    bus.publish(ServerEvent::LibraryUpdated);
}

#[allow(clippy::too_many_arguments)]
fn probe_worker_loop(
    pool: &Pool,
    ffprobe_present: bool,
    sources: &Sources,
    queue: &Arc<Mutex<Vec<ProbeJob>>>,
    done: &Arc<AtomicUsize>,
    bus: &Bus,
    activity: &Activity,
    total: usize,
) {
    loop {
        let job = match queue.lock().unwrap().pop() {
            Some(j) => j,
            None => break,
        };
        if let Err(e) = probe_one(
            pool,
            ffprobe_present,
            sources,
            bus,
            &job.file_id,
            &job.abs_path,
            &job.item_id,
        ) {
            if e.downcast_ref::<Deferred>().is_some() {
                tracing::debug!(file = %job.file_id, error = %e, "probe deferred");
            } else {
                warn!(file = %job.file_id, error = %e, "failed to probe; it stays unprobed for a later pass");
            }
        }
        let n = done.fetch_add(1, Ordering::Relaxed) + 1;
        activity::probe_progress(activity, n);
        if n.is_multiple_of(25) {
            bus.publish(ServerEvent::ProbeProgress { done: n, total });
        }
    }
}

/// Store the stream columns (+ `probed=1`), derive intro/credits markers from any
/// embedded chapters, and emit `ItemUpdated` when this is the item's first probed
/// file. Shared by the background probe pass and the `pipeline.probe` stage.
///
/// A probe a signal took writes nothing: the file keeps `probed = 0` so a later
/// pass reads it, rather than standing as a title with no streams for good. The
/// same holds for a stub whose source did not answer, for a title that could not
/// be fetched, and for one still being cached on a build that cannot read the
/// network: each is an `Err` (the last two a [`Deferred`]) that has nothing to do
/// with the file, and the pipeline retries it. A stub whose
/// source answered something the core refuses is recorded as unreadable.
pub fn probe_one(
    pool: &Pool,
    ffprobe: bool,
    sources: &Sources,
    bus: &Bus,
    file_id: &str,
    abs_path: &str,
    item_id: &str,
) -> anyhow::Result<()> {
    let first_for_item = db::item_has_probed_file(pool, item_id)
        .map(|has| !has)
        .unwrap_or(true);
    let resolved = match sources.resolve_for_probe(abs_path) {
        Ok(resolved) => resolved,
        Err(crate::services::source::SourceError::Preparing) => return Err(Deferred::Preparing.into()),
        Err(error) if error.is_transient() => return Err(error.into()),
        Err(error) => return store_unreadable(pool, bus, file_id, item_id, first_for_item, &error.to_string()),
    };
    let container = resolved.container().unwrap_or_default();
    let result = match probe_outcome(&resolved.input, &container, ffprobe) {
        Probed::Done(result) => *result,
        Probed::Killed => {
            warn!(file = %file_id, "ffprobe was killed before it answered; leaving the file unprobed");
            return Ok(());
        }
        Probed::Network => return Err(Deferred::Network.into()),
    };
    db::set_file_probe(
        pool,
        file_id,
        &FileProbe {
            duration_ms: result.duration_ms,
            video: result.video.as_ref(),
            audio: result.audio.as_ref(),
            audio_tracks: &result.audio_tracks,
            subtitles: &result.subtitles,
            unreadable: result.unreadable.as_deref(),
        },
    )?;
    for (kind, start, end) in super::markers_from_chapters(&result.chapters, result.duration_ms) {
        let _ = db::set_marker(pool, item_id, kind, start, end, "chapters");
    }
    if first_for_item {
        bus.publish(ServerEvent::ItemUpdated {
            id: item_id.to_string(),
        });
    }
    Ok(())
}

fn store_unreadable(
    pool: &Pool,
    bus: &Bus,
    file_id: &str,
    item_id: &str,
    first_for_item: bool,
    reason: &str,
) -> anyhow::Result<()> {
    db::set_file_probe(
        pool,
        file_id,
        &FileProbe {
            unreadable: Some(reason),
            ..FileProbe::default()
        },
    )?;
    if first_for_item {
        bus.publish(ServerEvent::ItemUpdated {
            id: item_id.to_string(),
        });
    }
    Ok(())
}
