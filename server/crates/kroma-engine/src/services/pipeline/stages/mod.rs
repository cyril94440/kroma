//! The concrete pipeline stages: each owns a `Stage` descriptor plus its
//! `enumerate`/`process`, wrapping the existing processing code (ffmpeg,
//! chromaprint) so only iteration/skip/retry moves onto the ledger.

mod common;

pub mod credits;
pub mod embed;
pub mod loudness;
pub mod markers;
pub mod metadata;
pub mod probe;
pub mod storyboard;
pub mod subtitles;

#[cfg(test)]
mod remote_tests;

/// `sig` for a stage that reads a whole file, and whose subject is out of scope
/// while the file would have to be streamed: a stub whose source holds no
/// complete local copy yet (or does not answer) is signed
/// [`crate::db::pipeline::UNREADABLE_SIG`], so no task is created for it, and
/// one that already has a task is left as it is. The stage picks it up on the
/// drain after the copy lands, with nothing retrying in between. A cached stub
/// is signed by the stub and by the copy, so a copy evicted and cached again
/// is a changed input and a task that failed on the first one runs again.
pub(crate) fn when_cached(state: &crate::state::SharedState, abs: &str, sig: String) -> String {
    if !crate::services::source::is_stub(std::path::Path::new(abs)) {
        return sig;
    }
    match state.sources.local(abs) {
        Ok(Some(copy)) => format!("{sig}|{}", copy_identity(&copy)),
        _ => crate::db::pipeline::UNREADABLE_SIG.to_string(),
    }
}

fn copy_identity(copy: &std::path::Path) -> String {
    let Ok(meta) = std::fs::metadata(copy) else {
        return String::new();
    };
    let nanos = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    format!("{nanos}:{}", meta.len())
}

/// The file a whole-file stage reads for the library file at `abs`: itself, or
/// a stub's complete local copy. An `Err` for a stub that lost its copy between
/// enumeration and now, which the ledger retries.
pub(crate) fn whole_file(
    state: &crate::state::SharedState,
    abs: &str,
) -> anyhow::Result<std::path::PathBuf> {
    state
        .sources
        .local(abs)?
        .ok_or_else(|| anyhow::anyhow!("{abs} is streamed and has no cached copy to read yet"))
}

/// A cheap change-signature for a file: `mtime:size`. Changes when the file is
/// replaced, so the ledger re-queues that subject. Returns
/// [`crate::db::pipeline::UNREADABLE_SIG`] when the file can't be stat'd (e.g. the
/// media mount is briefly offline), which `reconcile` treats as "leave the task
/// alone" rather than a changed input, so a flapping mount does not re-queue the
/// whole library.
pub(crate) fn sig_for_path(abs: &str) -> String {
    match std::fs::metadata(abs) {
        Ok(m) => {
            let mtime = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            format!("{mtime}:{}", m.len())
        }
        Err(_) => crate::db::pipeline::UNREADABLE_SIG.to_string(),
    }
}
