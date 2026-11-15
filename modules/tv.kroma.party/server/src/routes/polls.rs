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

pub fn routes<S>() -> Router<S>
where
    S: HostCtx + Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/poll", get(current::<S>))
        .route("/poll/start", post(start::<S>))
        .route("/poll/vote", post(vote::<S>))
        .route("/poll/close", post(close::<S>))
}

async fn current<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
) -> Result<Response, Response> {
    let who = actor(&host, &party, &user)?;
    let snap = work(move || party.snapshot(&who)).await?;
    Ok(Json(json!({ "poll": snap["poll"], "lastPoll": snap["lastPoll"] })).into_response())
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct StartBody {
    question: String,
    kind: String,
    options: Vec<String>,
    duration_s: Option<i64>,
}

async fn start<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
    Json(body): Json<StartBody>,
) -> Result<Response, Response> {
    admin(&host, &party, &user)?;
    work(move || party.start_poll(&body.question, &body.kind, &body.options, body.duration_s))
        .await?
        .map(|view| Json(json!({ "poll": view })).into_response())
        .map_err(|code| refuse(StatusCode::BAD_REQUEST, &code))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct VoteBody {
    value: String,
}

async fn vote<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
    Json(body): Json<VoteBody>,
) -> Result<Response, Response> {
    let who = actor(&host, &party, &user)?;
    work(move || party.vote_poll(&who.person, &body.value))
        .await?
        .map(|_| ok())
        .map_err(|code| refuse(StatusCode::BAD_REQUEST, code))
}

async fn close<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
) -> Result<Response, Response> {
    admin(&host, &party, &user)?;
    work(move || party.close_poll("closedByAdmin"))
        .await?
        .map(|view| Json(json!({ "results": view })).into_response())
        .map_err(|code| refuse(StatusCode::CONFLICT, code))
}
