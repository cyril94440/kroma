use std::sync::Arc;

use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use kroma_module_sdk::host::{AuthUser, HostCtx};

use super::{actor, admin, ok, refuse, work};
use crate::party::Party;
use crate::sync::{Beat, FineDone, Recal};

pub fn routes<S>() -> Router<S>
where
    S: HostCtx + Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/sync/beat", post(beat::<S>))
        .route("/sync/fine-done", post(fine_done::<S>))
        .route("/sync/recalibration", post(recalibration::<S>))
        .route("/sync/kick", post(kick::<S>))
        .route("/sync/go-live", post(go_live::<S>))
        .route("/sync/fine-sync", post(fine_sync::<S>))
        .route("/sync/info", get(info::<S>))
}

#[derive(Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct BeatBody {
    enabled: bool,
    delay: f64,
    paused: bool,
    pinned: bool,
    blocked: bool,
}

impl Default for BeatBody {
    fn default() -> Self {
        Self { enabled: true, delay: 0.0, paused: false, pinned: false, blocked: false }
    }
}

async fn beat<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
    Json(b): Json<BeatBody>,
) -> Result<Response, Response> {
    let who = actor(&host, &user)?;
    let delay = if b.delay.is_finite() { b.delay } else { 0.0 };
    work(move || {
        party.sync_beat(
            &who.person,
            Beat { enabled: b.enabled, delay, paused: b.paused, pinned: b.pinned, blocked: b.blocked },
        )
    })
    .await?;
    Ok(ok())
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct FineBody {
    ms: f64,
    seeks: f64,
    abandoned: bool,
    solo: bool,
}

async fn fine_done<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
    Json(b): Json<FineBody>,
) -> Result<Response, Response> {
    let who = actor(&host, &user)?;
    let done = FineDone { ms: b.ms.round() as i64, seeks: b.seeks.round() as i64, abandoned: b.abandoned, solo: b.solo };
    work(move || party.sync_fine_done(&who.person, done)).await?;
    Ok(ok())
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RecalBody {
    gap: f64,
    target: f64,
    arrival: bool,
    blocked: bool,
}

async fn recalibration<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
    Json(b): Json<RecalBody>,
) -> Result<Response, Response> {
    let who = actor(&host, &user)?;
    work(move || {
        party.sync_recalibration(&who.person, Recal { gap: b.gap, target: b.target, arrival: b.arrival, blocked: b.blocked })
    })
    .await?;
    Ok(ok())
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct KickBody {
    user_id: String,
    reason: String,
}

async fn kick<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
    Json(b): Json<KickBody>,
) -> Result<Response, Response> {
    admin(&host, &user)?;
    work(move || party.kick(&b.user_id, &b.reason))
        .await?
        .map(|disruptions| Json(json!({ "kicked": true, "disruptions": disruptions })).into_response())
        .map_err(|code| refuse(StatusCode::CONFLICT, code))
}

async fn go_live<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
) -> Result<Response, Response> {
    let who = admin(&host, &user)?;
    work(move || party.sync_go_live(&who.person.user_id))
        .await?
        .map(|members| Json(json!({ "members": members })).into_response())
        .map_err(|code| refuse(StatusCode::CONFLICT, code))
}

async fn fine_sync<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
) -> Result<Response, Response> {
    let who = admin(&host, &user)?;
    work(move || party.sync_fine_sync(&who.person.user_id))
        .await?
        .map(|_| ok())
        .map_err(|code| refuse(StatusCode::CONFLICT, code))
}

async fn info<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
) -> Result<Response, Response> {
    admin(&host, &user)?;
    Ok(Json(party.sync_info()).into_response())
}
