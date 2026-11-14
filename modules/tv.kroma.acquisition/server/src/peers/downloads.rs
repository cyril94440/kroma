//! The `download-grab` and `download-db` points this module calls: hand over a
//! release to grab, then work the ledger the import pass reads.

use serde::{Deserialize, Serialize};

use kroma_module_sdk::host::{call, call_raw, pinned_resolver, run_blocking, HostCtx, Resolver};
use kroma_module_sdk::http::Loopback;

pub const DOWNLOAD_GRAB: &str = "tv.kroma.torrents/grab";

pub const DOWNLOAD_DB: &str = "tv.kroma.torrents/db";

/// Everything the queue needs to grab a release and let it be imported.
/// `upgrade` means it replaces media already on disk, so the import takes the
/// destination and clears what it superseded instead of landing beside it.
#[derive(Debug, Clone, Default, Serialize)]
pub struct GrabSpec {
    pub magnet_or_url: String,
    pub kind: String,
    pub tmdb_id: u64,
    pub title: Option<String>,
    pub year: Option<u32>,
    pub season: Option<u32>,
    pub episodes: Option<Vec<u32>>,
    pub release_title: String,
    pub indexer_id: Option<String>,
    pub size_bytes: Option<u64>,
    pub score: Option<i32>,
    pub score_breakdown: Option<String>,
    pub request_id: Option<String>,
    pub wanted_ids: Vec<String>,
    pub only_files: Option<Vec<usize>>,
    pub details_url: Option<String>,
    pub upgrade: bool,
}

/// A ledger row, as the import pass reads it. Tolerant: the module that keeps the
/// ledger is released separately, so a field it adds is ignored here and one it
/// stops sending defaults rather than failing the whole import sweep.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct DownloadRow {
    pub id: String,
    pub request_id: Option<String>,
    pub kind: String,
    pub tmdb_id: u64,
    pub title: Option<String>,
    pub year: Option<u32>,
    pub season: Option<u32>,
    pub episodes: Option<Vec<u32>>,
    pub release_title: String,
    pub indexer_id: Option<String>,
    pub magnet_or_url: String,
    pub size_bytes: Option<u64>,
    pub score: Option<i32>,
    pub status: String,
    pub save_path: Option<String>,
    pub error: Option<String>,
    pub details_url: Option<String>,
    pub only_files: Option<Vec<usize>>,
    pub upgrade: bool,
    pub content_rel_paths: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct TorrentFileEntry {
    pub index: usize,
    pub path: String,
    pub size_bytes: u64,
}

pub fn grab(host: &dyn HostCtx, spec: &GrabSpec) -> anyhow::Result<DownloadRow> {
    call(&grabber(host)?, &format!("{DOWNLOAD_GRAB}/grab"), spec)
}

/// A torrent's files without downloading it, so the admin can select before
/// committing.
pub fn list_files(
    host: &dyn HostCtx,
    magnet_or_url: &str,
) -> anyhow::Result<Vec<TorrentFileEntry>> {
    call(
        &grabber(host)?,
        &format!("{DOWNLOAD_GRAB}/list-files"),
        &serde_json::json!({ "magnet_or_url": magnet_or_url }),
    )
}

/// Whether the kill switch currently allows new grabs. A missing engine reads as
/// closed, because a grab that cannot be handed anywhere is not a grab.
pub fn gate_open(host: &dyn HostCtx) -> bool {
    let Ok(resolve) = grabber(host) else {
        return false;
    };
    call_raw(
        &resolve,
        &format!("{DOWNLOAD_GRAB}/gate-open"),
        &serde_json::json!({}),
    )
    .unwrap_or(false)
}

/// A release offered to an instant grab, under the key the answer names it by.
#[derive(Debug, Clone, Serialize)]
pub struct InstantRelease {
    pub key: String,
    pub spec: GrabSpec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstantStatus {
    Grabbed,
    Nothing,
    Unsupported,
    Failed,
    #[serde(other)]
    Unknown,
}

/// What the queue's instant grab came to: the release it kept and the ledger
/// row, already completed, when it kept one.
#[derive(Debug, Clone, Deserialize)]
pub struct InstantOutcome {
    pub status: InstantStatus,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub download: Option<DownloadRow>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub deterministic: bool,
}

/// Asks the queue to keep the best of the ranked releases that its download
/// client already holds. The queue leaves nothing behind for the others.
pub fn grab_instant(
    host: &dyn HostCtx,
    releases: &[InstantRelease],
    budget: std::time::Duration,
) -> anyhow::Result<InstantOutcome> {
    let resolve = grabber(host)?;
    let (base, token) = resolve().ok_or_else(|| anyhow::anyhow!("no module contributes this point"))?;
    let secs = (budget.as_secs_f64().ceil() as u32 + 2).max(2);
    let answer: Result<InstantOutcome, String> = Loopback::new()
        .max_time(secs)
        .header("authorization", format!("Bearer {token}"))
        .post_json(
            &format!("{base}/_port/{DOWNLOAD_GRAB}/grab-instant"),
            &serde_json::json!({ "releases": releases, "budget_ms": budget.as_millis() as u64 }),
        )?
        .ensure_ok()?
        .json()?;
    answer.map_err(|e| anyhow::anyhow!(e))
}

/// Whether the preferred download client can say which releases it holds.
/// Absent queue or a failed call read as no.
pub fn instant_capable(host: &dyn HostCtx) -> bool {
    let Ok(resolve) = grabber(host) else {
        return false;
    };
    call_raw(
        &resolve,
        &format!("{DOWNLOAD_GRAB}/instant-capable"),
        &serde_json::json!({}),
    )
    .unwrap_or(false)
}

/// Kick a freshly recorded row into the engine. Best effort: the row is already in
/// the ledger, so the engine picks it up on its next sweep either way.
pub fn activate(host: &dyn HostCtx, id: &str) {
    if let Err(e) = lifecycle(host, "activate", id, false) {
        tracing::warn!(target: "acquisition", download = %id, "could not start the download: {e:#}");
    }
}

/// [`activate`], once the caller has answered: an engine add is slow. Needs a
/// tokio runtime, which every route and job here runs under.
pub fn activate_later<H: HostCtx + Send + Sync + 'static>(host: H, id: String) {
    tokio::spawn(async move {
        let _ = run_blocking(move || activate(&host, &id)).await;
    });
}

/// Free a download's data and stop seeding, once its files were imported.
pub fn drop_data(host: &dyn HostCtx, id: &str) {
    if let Err(e) = lifecycle(host, "drop-data", id, false) {
        tracing::warn!(target: "acquisition", download = %id, "could not free the download: {e:#}");
    }
}

/// [`drop_data`] for a release that was turned down: the engine also removes
/// whatever it keeps on its side for it, where it made it.
pub fn discard(host: &dyn HostCtx, id: &str) {
    if let Err(e) = lifecycle(host, "drop-data", id, true) {
        tracing::warn!(target: "acquisition", download = %id, "could not discard the download: {e:#}");
    }
}

fn lifecycle(host: &dyn HostCtx, method: &str, id: &str, discard: bool) -> anyhow::Result<bool> {
    call(
        &grabber(host)?,
        &format!("{DOWNLOAD_GRAB}/{method}"),
        &serde_json::json!({ "id": id, "discard": discard }),
    )
}

/// Rows the engine finished and nothing has imported yet.
pub fn completed(host: &dyn HostCtx) -> anyhow::Result<Vec<DownloadRow>> {
    call(
        &ledger(host)?,
        &format!("{DOWNLOAD_DB}/completed"),
        &serde_json::json!({}),
    )
}

/// Record where an import landed a row's files.
pub fn mark_imported(
    host: &dyn HostCtx,
    id: &str,
    paths: &[String],
    now_ms: i64,
) -> anyhow::Result<()> {
    call(
        &ledger(host)?,
        &format!("{DOWNLOAD_DB}/mark-imported"),
        &serde_json::json!({ "id": id, "paths": paths, "now_ms": now_ms }),
    )
}

/// Move a row to `status`, with the reason when it failed.
pub fn set_status(
    host: &dyn HostCtx,
    id: &str,
    status: &str,
    error: Option<&str>,
) -> anyhow::Result<bool> {
    call(
        &ledger(host)?,
        &format!("{DOWNLOAD_DB}/set-status"),
        &serde_json::json!({ "id": id, "status": status, "error": error }),
    )
}

fn grabber(host: &dyn HostCtx) -> anyhow::Result<Resolver> {
    resolve(host, DOWNLOAD_GRAB)
}

fn ledger(host: &dyn HostCtx) -> anyhow::Result<Resolver> {
    resolve(host, DOWNLOAD_DB)
}

fn resolve(host: &dyn HostCtx, point: &str) -> anyhow::Result<Resolver> {
    pinned_resolver(host, point, None)
        .ok_or_else(|| anyhow::anyhow!("no module answers the {point} point"))
}

#[cfg(test)]
mod tests;
