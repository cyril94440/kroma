use std::sync::Arc;

use axum::extract::{Extension, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use kroma_module_sdk::host::{AuthUser, HostCtx};

use super::{actor, admin, ok, refuse, work};
use crate::party::{Party, Request};

pub fn routes<S>() -> Router<S>
where
    S: HostCtx + Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/vote", post(cast::<S>))
        .route("/request-switch", post(request_switch::<S>))
        .route("/votes", get(votes::<S>))
        .route("/channels", get(channels::<S>))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct CastBody {
    approve: bool,
}

async fn cast<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
    Json(body): Json<CastBody>,
) -> Result<Response, Response> {
    let who = actor(&host, &party, &user)?;
    work(move || party.actor_vote(&who, body.approve))
        .await?
        .map(|_| ok())
        .map_err(|code| refuse(StatusCode::CONFLICT, code))
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct SwitchBody {
    channel_id: String,
    channel_name: Option<String>,
    force: bool,
}

async fn request_switch<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
    Json(body): Json<SwitchBody>,
) -> Result<Response, Response> {
    let who = actor(&host, &party, &user)?;
    if body.channel_id.trim().is_empty() {
        return Err(refuse(StatusCode::BAD_REQUEST, "channelIdRequired"));
    }
    let force = body.force && party.may_bypass(&who);
    let forced_denied = body.force && !force;
    let result = work(move || {
        party.request_switch(Request {
            channel_id: &body.channel_id,
            channel_name: body.channel_name.as_deref(),
            requester: Some(who.person.clone()),
            note: "",
            force,
        })
    })
    .await?;
    Ok(Json(json!({
        "outcome": result.outcome,
        "voteId": result.vote_id,
        "reason": result.reason,
        "forceDenied": forced_denied,
    }))
    .into_response())
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct VotesQuery {
    limit: Option<u32>,
}

async fn votes<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
    Query(q): Query<VotesQuery>,
) -> Result<Response, Response> {
    admin(&host, &party, &user)?;
    let rows = work(move || party.recent_votes(q.limit.unwrap_or(50))).await?;
    Ok(Json(json!({ "votes": rows })).into_response())
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ChannelsQuery {
    q: String,
}

async fn channels<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
    Query(query): Query<ChannelsQuery>,
) -> Result<Response, Response> {
    actor(&host, &party, &user)?;
    let found = work(move || party.find_channels(&query.q)).await?;
    Ok(Json(json!({ "channels": found })).into_response())
}
