//! The `download-client` point this module answers, under the instance
//! `alldebrid`. Every call carries the client it is for; this process holds no
//! record of which of the operator's clients it is serving.

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use kroma_module_sdk::host::{port_reply, run_blocking, HostCtx};

use crate::settings::CacheSettings;
use crate::{AllDebrid, ClientDef, DATA_DIR};

pub fn routes<S: HostCtx + Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route("/_port/tv.kroma.torrents/client/test", post(test::<S>))
        .route("/_port/tv.kroma.torrents/client/add", post(add::<S>))
        .route("/_port/tv.kroma.torrents/client/status", post(status::<S>))
        .route("/_port/tv.kroma.torrents/client/pause", post(pause::<S>))
        .route("/_port/tv.kroma.torrents/client/resume", post(resume::<S>))
        .route(
            "/_port/tv.kroma.torrents/client/reannounce",
            post(reannounce::<S>),
        )
        .route("/_port/tv.kroma.torrents/client/remove", post(remove::<S>))
        .route("/_port/tv.kroma.torrents/client/instant", post(instant::<S>))
        .route("/_port/source/locate", post(locate::<S>))
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Locating {
    #[serde(rename = "ref")]
    reference: String,
    warm: bool,
}

async fn locate<S: HostCtx + Clone + Send + Sync + 'static>(
    State(host): State<S>,
    Json(call): Json<Locating>,
) -> Json<Result<crate::locate::Located, String>> {
    port_reply(move || {
        crate::locate::locate(
            &host.data_dir().join(DATA_DIR),
            CacheSettings::read(&host),
            &call.reference,
            call.warm,
        )
    })
    .await
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct Client {
    url: String,
    username: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct Call {
    client: Client,
    #[serde(default)]
    args: Args,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Args {
    magnet_or_url: String,
    client_ref: String,
    delete_data: bool,
    candidates: Vec<InstantCandidate>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct InstantCandidate {
    key: String,
    magnet_or_url: String,
}

#[derive(Debug, Serialize)]
struct InstantReply {
    key: String,
    client_ref: String,
}

#[derive(Debug, Clone, Serialize)]
struct Status {
    client_ref: String,
    name: String,
    info_hash: Option<String>,
    progress: f64,
    state: String,
    down_bps: u64,
    up_bps: u64,
    peers: u32,
    peers_seen: u32,
    size_bytes: u64,
    save_path: Option<String>,
    files: Vec<String>,
    error: Option<String>,
    lifetime_downloaded_bytes: u64,
    lifetime_uploaded_bytes: u64,
}

fn engine<S: HostCtx>(host: &S, client: &Client) -> anyhow::Result<AllDebrid> {
    AllDebrid::new(
        &ClientDef {
            url: client.url.clone(),
            username: client.username.clone(),
            password: client.password.clone(),
        },
        &host.data_dir().join(DATA_DIR),
    )
}

async fn test<S: HostCtx + Clone + Send + Sync + 'static>(
    State(host): State<S>,
    Json(call): Json<Call>,
) -> Json<Result<String, String>> {
    port_reply(move || engine(&host, &call.client)?.test()).await
}

async fn add<S: HostCtx + Clone + Send + Sync + 'static>(
    State(host): State<S>,
    Json(call): Json<Call>,
) -> Json<Result<String, String>> {
    port_reply(move || engine(&host, &call.client)?.add(&call.args.magnet_or_url)).await
}

// Not the `Result` envelope: `None` is a magnet AllDebrid has forgotten, which
// the consumer reconciles. A failed call is a 502 instead, so a bad moment at
// AllDebrid never reads as a magnet that is gone.
async fn status<S: HostCtx + Clone + Send + Sync + 'static>(
    State(host): State<S>,
    Json(call): Json<Call>,
) -> Result<Json<Option<Status>>, (StatusCode, String)> {
    let asked = run_blocking(move || {
        engine(&host, &call.client)?
            .status(&call.args.client_ref)
            .map(|found| found.map(wire))
    })
    .await;
    match asked {
        Ok(Ok(found)) => Ok(Json(found)),
        Ok(Err(error)) => Err((StatusCode::BAD_GATEWAY, format!("{error:#}"))),
        Err(error) => Err((StatusCode::INTERNAL_SERVER_ERROR, error.to_string())),
    }
}

fn wire(s: crate::TorrentStatus) -> Status {
    Status {
        client_ref: s.client_ref,
        name: s.name,
        info_hash: s.info_hash,
        progress: s.progress,
        state: serde_json::to_value(s.state)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_else(|| "error".into()),
        down_bps: s.down_bps,
        up_bps: s.up_bps,
        peers: s.peers,
        peers_seen: s.peers_seen,
        size_bytes: s.size_bytes,
        save_path: s.save_path,
        files: s.files,
        error: s.error,
        lifetime_downloaded_bytes: s.lifetime_downloaded_bytes,
        lifetime_uploaded_bytes: s.lifetime_uploaded_bytes,
    }
}

async fn instant<S: HostCtx + Clone + Send + Sync + 'static>(
    State(host): State<S>,
    Json(call): Json<Call>,
) -> Json<Result<Option<InstantReply>, String>> {
    port_reply(move || {
        let candidates: Vec<crate::Candidate> = call
            .args
            .candidates
            .into_iter()
            .map(|c| crate::Candidate { key: c.key, magnet_or_url: c.magnet_or_url })
            .collect();
        Ok(engine(&host, &call.client)?
            .instant(&candidates)?
            .map(|found| InstantReply { key: found.key, client_ref: found.client_ref }))
    })
    .await
}

async fn pause<S: HostCtx + Clone + Send + Sync + 'static>(
    State(host): State<S>,
    Json(call): Json<Call>,
) -> Json<Result<(), String>> {
    port_reply(move || engine(&host, &call.client)?.pause(&call.args.client_ref)).await
}

async fn resume<S: HostCtx + Clone + Send + Sync + 'static>(
    State(host): State<S>,
    Json(call): Json<Call>,
) -> Json<Result<(), String>> {
    port_reply(move || engine(&host, &call.client)?.resume(&call.args.client_ref)).await
}

async fn reannounce<S: HostCtx + Clone + Send + Sync + 'static>(
    State(host): State<S>,
    Json(call): Json<Call>,
) -> Json<Result<(), String>> {
    port_reply(move || engine(&host, &call.client)?.reannounce(&call.args.client_ref)).await
}

async fn remove<S: HostCtx + Clone + Send + Sync + 'static>(
    State(host): State<S>,
    Json(call): Json<Call>,
) -> Json<Result<(), String>> {
    port_reply(move || {
        engine(&host, &call.client)?.remove(&call.args.client_ref, call.args.delete_data)
    })
    .await
}

#[cfg(test)]
mod tests;
