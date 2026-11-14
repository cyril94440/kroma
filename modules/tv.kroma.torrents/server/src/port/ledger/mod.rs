//! The `download-grab` and `download-db` points this module answers: the grab
//! spec a consumer hands over, the ledger row it gets back, and the reads and
//! writes an import pass needs.

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use std::time::Duration;

use kroma_module_sdk::host::{port_reply, run_blocking, HostStorage};

use super::manager;
use crate::db::DownloadRow;
use crate::{Begun, DownloadDb, GrabSpec, InstantOutcome, InstantRelease, Settle, TorrentFileEntry};

const DEFAULT_INSTANT_BUDGET_MS: u64 = 20_000;
const SETTLE_EVERY: Duration = Duration::from_millis(250);

pub const DOWNLOAD_GRAB: &str = "tv.kroma.torrents/grab";

pub const DOWNLOAD_DB: &str = "tv.kroma.torrents/db";

pub fn routes<S: HostStorage + Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route("/_port/tv.kroma.torrents/grab/grab", post(grab::<S>))
        .route(
            "/_port/tv.kroma.torrents/grab/list-files",
            post(list_files::<S>),
        )
        .route(
            "/_port/tv.kroma.torrents/grab/gate-open",
            post(gate_open::<S>),
        )
        .route(
            "/_port/tv.kroma.torrents/grab/activate",
            post(activate::<S>),
        )
        .route(
            "/_port/tv.kroma.torrents/grab/drop-data",
            post(drop_data::<S>),
        )
        .route(
            "/_port/tv.kroma.torrents/grab/grab-instant",
            post(grab_instant::<S>),
        )
        .route(
            "/_port/tv.kroma.torrents/grab/instant-capable",
            post(instant_capable::<S>),
        )
        .route(
            "/_port/tv.kroma.torrents/db/completed",
            post(completed::<S>),
        )
        .route(
            "/_port/tv.kroma.torrents/db/mark-imported",
            post(mark_imported::<S>),
        )
        .route(
            "/_port/tv.kroma.torrents/db/set-status",
            post(set_status::<S>),
        )
}

fn ledger<S: HostStorage>(host: &S) -> DownloadDb {
    DownloadDb::new(host.db().clone())
}

async fn grab<S: HostStorage + Clone + Send + Sync + 'static>(
    State(host): State<S>,
    Json(spec): Json<GrabSpec>,
) -> Json<Result<DownloadRow, String>> {
    port_reply(move || manager(&host)?.grab(&host, spec)).await
}

#[derive(Deserialize)]
struct InstantReq {
    releases: Vec<InstantRelease>,
    #[serde(default)]
    budget_ms: Option<u64>,
}

async fn grab_instant<S: HostStorage + Clone + Send + Sync + 'static>(
    State(host): State<S>,
    Json(req): Json<InstantReq>,
) -> Json<Result<InstantOutcome, String>> {
    let budget = Duration::from_millis(req.budget_ms.unwrap_or(DEFAULT_INSTANT_BUDGET_MS));
    Json(
        instant_flow(host, req.releases, budget)
            .await
            .map_err(|error| format!("{error:#}")),
    )
}

async fn instant_flow<S: HostStorage + Clone + Send + Sync + 'static>(
    host: S,
    releases: Vec<InstantRelease>,
    budget: Duration,
) -> anyhow::Result<InstantOutcome> {
    let began = {
        let host = host.clone();
        run_blocking(move || manager(&host)?.instant_begin(&host, releases, budget))
            .await
            .map_err(|error| anyhow::anyhow!("{error}"))??
    };
    let pending = match began {
        Begun::Done(outcome) => return Ok(outcome),
        Begun::Settling(pending) => pending,
    };
    let mut pending = Some(pending);
    loop {
        let (asking, held) = (host.clone(), pending.take().expect("a pending release"));
        let (held, step) = run_blocking(move || {
            let manager = manager(&asking)?;
            let step = manager.settle_step(&asking, &held);
            Ok::<_, anyhow::Error>((held, step))
        })
        .await
        .map_err(|error| anyhow::anyhow!("{error}"))??;
        match step {
            Settle::Waiting(last) if !held.expired() => {
                let _ = last;
                pending = Some(held);
                tokio::time::sleep(SETTLE_EVERY).await;
            }
            step => {
                let host = host.clone();
                return run_blocking(move || manager(&host)?.instant_finish(&host, held, step))
                    .await
                    .map_err(|error| anyhow::anyhow!("{error}"))?;
            }
        }
    }
}

// Not fallible: an absent manager is a client that cannot do it.
async fn instant_capable<S: HostStorage + Clone + Send + Sync + 'static>(
    State(host): State<S>,
) -> Json<bool> {
    Json(
        kroma_module_sdk::host::run_blocking(move || {
            manager(&host).map(|m| m.instant_capable()).unwrap_or(false)
        })
        .await
        .unwrap_or(false),
    )
}

#[derive(Deserialize)]
struct MagnetReq {
    magnet_or_url: String,
}

async fn list_files<S: HostStorage + Clone + Send + Sync + 'static>(
    State(host): State<S>,
    Json(req): Json<MagnetReq>,
) -> Json<Result<Vec<TorrentFileEntry>, String>> {
    port_reply(move || manager(&host)?.list_files(&host, &req.magnet_or_url)).await
}

// Not fallible: a closed gate and an absent manager both mean "do not grab".
async fn gate_open<S: HostStorage + Clone + Send + Sync + 'static>(
    State(host): State<S>,
) -> Json<bool> {
    Json(manager(&host).map(|m| m.gate_open()).unwrap_or(false))
}

#[derive(Deserialize)]
struct IdReq {
    id: String,
    #[serde(default)]
    discard: bool,
}

async fn activate<S: HostStorage + Clone + Send + Sync + 'static>(
    State(host): State<S>,
    Json(req): Json<IdReq>,
) -> Json<Result<bool, String>> {
    port_reply(move || {
        let manager = manager(&host)?;
        let Some(row) = ledger(&host).get(&req.id)? else {
            return Ok(false);
        };
        manager.activate(&host, &row);
        Ok(true)
    })
    .await
}

async fn drop_data<S: HostStorage + Clone + Send + Sync + 'static>(
    State(host): State<S>,
    Json(req): Json<IdReq>,
) -> Json<Result<bool, String>> {
    port_reply(move || {
        let manager = manager(&host)?;
        let Some(row) = ledger(&host).get(&req.id)? else {
            return Ok(false);
        };
        manager.drop_data(&row, req.discard);
        Ok(true)
    })
    .await
}

#[derive(Serialize)]
struct CompletedRow {
    #[serde(flatten)]
    row: DownloadRow,
    content_rel_paths: Vec<String>,
}

async fn completed<S: HostStorage + Clone + Send + Sync + 'static>(
    State(host): State<S>,
) -> Json<Result<Vec<CompletedRow>, String>> {
    port_reply(move || {
        ledger(&host)
            .completed()?
            .into_iter()
            .map(|row| {
                let content_rel_paths = crate::db::download_contents(host.store(), &row.id)?;
                Ok(CompletedRow {
                    row,
                    content_rel_paths,
                })
            })
            .collect()
    })
    .await
}

#[derive(Deserialize)]
struct ImportedReq {
    id: String,
    paths: Vec<String>,
    now_ms: i64,
}

async fn mark_imported<S: HostStorage + Clone + Send + Sync + 'static>(
    State(host): State<S>,
    Json(req): Json<ImportedReq>,
) -> Json<Result<(), String>> {
    port_reply(move || ledger(&host).mark_imported(&req.id, &req.paths, req.now_ms)).await
}

#[derive(Deserialize)]
struct StatusReq {
    id: String,
    status: String,
    #[serde(default)]
    error: Option<String>,
}

async fn set_status<S: HostStorage + Clone + Send + Sync + 'static>(
    State(host): State<S>,
    Json(req): Json<StatusReq>,
) -> Json<Result<bool, String>> {
    port_reply(move || ledger(&host).set_status(&req.id, &req.status, req.error.as_deref())).await
}

#[cfg(test)]
mod tests;
