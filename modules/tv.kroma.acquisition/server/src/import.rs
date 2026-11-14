//! Import: move completed downloads into the library with the configured
//! Sonarr/Radarr-style naming (see the `kroma-naming` library), so the
//! regular scan/enrich/pipeline takes over. Hardlink first (the torrent keeps
//! seeding from its download folder for free), copy across filesystems.

use std::path::Path;

use anyhow::{anyhow, Result};

use crate::peers::downloads::DownloadRow;
use kroma_module_sdk::db;
use kroma_module_sdk::host::HostStorage;
use kroma_module_sdk::primitives::now_ms;

mod claim;
mod content;
mod ledger;
mod naming;
mod place;
mod recycle;
mod remote;
mod replace;
mod row;
mod subtitles;

pub use recycle::empty_recycle_bin;
pub use row::{Landing, Placed};

use content::FilesGone;
use ledger::{until_it_lands, BUSY_BACKOFF};
use row::{place_row, Replaced};

/// Takes files this module wrote out of the library again, each through the
/// recycle bin when one is set. Only the exact paths given go: a subtitle or a
/// video that was already there is never found by looking beside them.
pub fn retire_written(host: &dyn kroma_module_sdk::host::HostCtx, paths: &[String]) -> usize {
    paths
        .iter()
        .filter(|path| match recycle::retire(host, Path::new(path)) {
            Ok(_) => true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(e) => {
                tracing::warn!(file = %path, "a rejected file stayed behind: {e}");
                false
            }
        })
        .count()
}

#[derive(Debug, Default)]
pub struct ImportSummary {
    pub imported: usize,
    pub files: usize,
    pub failed: usize,
}

pub(crate) fn finalize_import<S: HostStorage>(state: &S, row: &DownloadRow, written: &[String]) {
    use kroma_module_sdk::requests::lifecycle::{on_download_imported, Imported};
    if row.tmdb_id != 0 {
        let imported = Imported {
            kind: crate::auto::title_kind(&row.kind),
            tmdb_id: row.tmdb_id,
            season: row.season,
            episodes: row.episodes.as_deref(),
            request_id: row.request_id.as_deref(),
        };
        if let Err(e) = until_it_lands(&BUSY_BACKOFF, "post-import ledger update", || {
            on_download_imported(state, &imported)
        }) {
            tracing::warn!(download = %row.id, error = %format!("{e:#}"), "post-import ledger update failed");
        }
        record_file_tmdb(state, row.tmdb_id, written);
    }
    record_releases(state, &row.release_title, written);
    if state.setting_bool("acqDeleteAfterImport", false) {
        crate::peers::downloads::drop_data(state, &row.id);
    }
}

fn record_releases<S: HostStorage>(state: &S, release_title: &str, written: &[String]) {
    for path in written {
        let key = canonical(path);
        if let Err(e) = crate::rules::store::record_release(state.store(), &key, release_title, now_ms()) {
            tracing::debug!(path = %key, error = %format!("{e:#}"), "could not record the release name");
        }
    }
}

fn canonical(path: &str) -> String {
    kroma_module_sdk::host::files::canonical(path).unwrap_or_else(|| path.to_string())
}

fn record_file_tmdb<S: HostStorage>(state: &S, tmdb_id: u64, written: &[String]) {
    for path in written {
        // Keyed by canonical path to match the scanner's `files.abs_path`, so the
        // enrichment join lines up regardless of title-parse differences.
        let key = canonical(path);
        if let Err(e) = db::set_file_tmdb(state.db(), &key, tmdb_id) {
            tracing::debug!(path = %key, error = %format!("{e:#}"), "could not record file tmdbId");
        }
    }
}

/// Import every `completed` download. Failures land on the row's `error`
/// (visible in the queue) without blocking the others.
pub fn import_pass<S: HostStorage>(state: &S, log: &dyn Fn(String)) -> Result<ImportSummary> {
    let ready = crate::peers::downloads::completed(state)?;
    let mut summary = ImportSummary::default();
    for row in ready {
        if crate::watch_now::is_held_for_watch_now(state, &row) {
            continue;
        }
        let Some(_claim) = claim::Claim::take(state.store(), &row.id) else {
            continue;
        };
        match import_one(state, &row) {
            Ok(paths) => {
                log(format!(
                    "imported \"{}\" ({} files)",
                    row.release_title,
                    paths.len()
                ));
                summary.imported += 1;
                summary.files += paths.len();
                if let Err(e) = record_import(state, &row, &paths) {
                    tracing::warn!(download = %row.id, error = %format!("{e:#}"), "could not mark the download imported");
                }
            }
            Err(e) => {
                log(format!(
                    "import failed for \"{}\": {e:#}",
                    row.release_title
                ));
                summary.failed += 1;
                refuse_import(state, &row, &e)?;
            }
        }
    }
    if summary.imported > 0 {
        state.trigger_job("library.scan", "acquisition-import");
    }
    Ok(summary)
}

fn record_import<S: HostStorage>(state: &S, row: &DownloadRow, paths: &[String]) -> Result<()> {
    until_it_lands(&BUSY_BACKOFF, "mark imported", || {
        crate::peers::downloads::mark_imported(state, &row.id, paths, now_ms())
    })?;
    finalize_import(state, row, paths);
    Ok(())
}

fn refuse_import<S: HostStorage>(state: &S, row: &DownloadRow, error: &anyhow::Error) -> Result<()> {
    let status = if error.is::<FilesGone>() {
        "removed"
    } else {
        "completed"
    };
    crate::peers::downloads::set_status(state, &row.id, status, Some(&format!("import: {error:#}")))?;
    Ok(())
}

fn import_one<S: HostStorage>(state: &S, row: &DownloadRow) -> Result<Vec<String>> {
    Ok(place_row(state, row)?
        .videos
        .into_iter()
        .map(|p| p.path)
        .collect())
}

/// Places one `completed` download's files for a watch-now, without telling
/// the ledger or the request anything: that is [`finalize_placed`]'s job once
/// the caller has judged the file. Says where each file landed, and which of
/// them this call wrote. `None` when the import pass, or another call, holds
/// the download. Failures land on the row like the pass's do.
pub fn place_download<S: HostStorage>(
    state: &S,
    download_id: &str,
) -> Result<Option<(DownloadRow, Landing)>> {
    let Some(claim) = claim::Claim::take(state.store(), download_id) else {
        return Ok(None);
    };
    let row = crate::peers::downloads::completed(state)?
        .into_iter()
        .find(|row| row.id == download_id)
        .ok_or_else(|| anyhow!("download {download_id} is not waiting to be imported"))?;
    let landing = match place_row(state, &row) {
        Ok(landing) => landing,
        Err(e) => {
            refuse_import(state, &row, &e)?;
            return Err(e);
        }
    };
    drop(claim);
    state.trigger_job("library.scan", "acquisition-import");
    Ok(Some((row, landing)))
}

/// What a download recorded as imported: the files and nothing else, read back
/// for a download another import already finished.
pub fn recorded_paths<S: HostStorage>(state: &S, download_id: &str) -> Result<Option<Vec<String>>> {
    let conn = state.db().get()?;
    let found: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT status, imported_paths FROM downloads WHERE id = ?1",
            [download_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    Ok(found.and_then(|(status, paths)| {
        (status == "imported")
            .then(|| paths.and_then(|text| serde_json::from_str(&text).ok()))
            .flatten()
    }))
}

/// Records a placed download as imported and tells the request: available,
/// notified, ledgered.
pub fn finalize_placed<S: HostStorage>(state: &S, row: &DownloadRow, paths: &[String]) -> Result<()> {
    record_import(state, row, paths)
}

#[cfg(test)]
pub(crate) fn hold_for_test<'a>(
    store: &'a kroma_module_sdk::db::Pool,
    download_id: &str,
) -> impl Drop + 'a {
    claim::Claim::take(store, download_id).expect("a free claim")
}

#[cfg(test)]
mod tests;
