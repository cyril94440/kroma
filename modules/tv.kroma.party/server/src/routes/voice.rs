use std::sync::Arc;

use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use kroma_module_sdk::host::{AuthUser, HostCtx};

use super::{actor, ok, refuse, work};
use crate::party::Party;
use crate::voice::{Signal, VoiceError};

pub fn routes<S>() -> Router<S>
where
    S: HostCtx + Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/voice/state", get(state::<S>))
        .route("/voice/ice", get(ice::<S>))
        .route("/voice/join", post(join::<S>))
        .route("/voice/leave", post(leave::<S>))
        .route("/voice/beat", post(beat::<S>))
        .route("/voice/mute", post(mute::<S>))
        .route("/voice/speaking", post(speaking::<S>))
        .route("/voice/signal", post(signal::<S>))
}

fn refusal(error: VoiceError) -> Response {
    let status = match error {
        VoiceError::BadDevice | VoiceError::SelfSignal | VoiceError::BadSignal => {
            StatusCode::BAD_REQUEST
        }
        VoiceError::Full => StatusCode::CONFLICT,
        VoiceError::RateLimited => StatusCode::TOO_MANY_REQUESTS,
        VoiceError::NotJoined | VoiceError::PeerGone => StatusCode::NOT_FOUND,
        VoiceError::Forbidden => StatusCode::FORBIDDEN,
        VoiceError::TooLarge => StatusCode::PAYLOAD_TOO_LARGE,
    };
    refuse(status, error.code())
}

async fn state<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
) -> Result<Response, Response> {
    actor(&host, &user)?;
    Ok(Json(party.voice_state()).into_response())
}

async fn ice<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
) -> Result<Response, Response> {
    let who = actor(&host, &user)?;
    work(move || party.voice_ice(&who.person.user_id))
        .await?
        .map(|answer| Json(answer).into_response())
        .map_err(refusal)
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct JoinBody {
    device_id: String,
    muted: bool,
    watching: bool,
}

async fn join<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
    Json(body): Json<JoinBody>,
) -> Result<Response, Response> {
    let who = actor(&host, &user)?;
    work(move || party.voice_join(&who, &body.device_id, body.muted, body.watching))
        .await?
        .map(|answer| Json(answer).into_response())
        .map_err(refusal)
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct PeerBody {
    peer_id: String,
}

async fn leave<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
    Json(body): Json<PeerBody>,
) -> Result<Response, Response> {
    let who = actor(&host, &user)?;
    work(move || party.voice_leave(&who.person.user_id, &body.peer_id))
        .await?
        .map(|()| ok())
        .map_err(refusal)
}

async fn beat<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
    Json(body): Json<PeerBody>,
) -> Result<Response, Response> {
    let who = actor(&host, &user)?;
    work(move || party.voice_beat(&who.person.user_id, &body.peer_id))
        .await?
        .map(|revision| Json(json!({ "ok": true, "revision": revision })).into_response())
        .map_err(refusal)
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct MuteBody {
    peer_id: String,
    muted: bool,
}

async fn mute<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
    Json(body): Json<MuteBody>,
) -> Result<Response, Response> {
    let who = actor(&host, &user)?;
    work(move || party.voice_mute(&who.person.user_id, &body.peer_id, body.muted))
        .await?
        .map(|()| ok())
        .map_err(refusal)
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct SpeakingBody {
    peer_id: String,
    speaking: bool,
}

async fn speaking<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
    Json(body): Json<SpeakingBody>,
) -> Result<Response, Response> {
    let who = actor(&host, &user)?;
    work(move || party.voice_speaking(&who.person.user_id, &body.peer_id, body.speaking))
        .await?
        .map(|()| ok())
        .map_err(refusal)
}

async fn signal<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
    body: Result<Json<Signal>, axum::extract::rejection::JsonRejection>,
) -> Result<Response, Response> {
    let who = actor(&host, &user)?;
    let Json(signal) = body.map_err(|_| refusal(VoiceError::BadSignal))?;
    work(move || party.voice_signal(&who.person.user_id, &signal))
        .await?
        .map(|n| Json(json!({ "ok": true, "n": n })).into_response())
        .map_err(refusal)
}
