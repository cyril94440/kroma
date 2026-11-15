use std::sync::Arc;

use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use kroma_module_sdk::host::{AuthUser, HostCtx};

use super::{admin, refuse, viewer, vote_status, work};
use crate::runtime::Sport;

pub fn routes<S: HostCtx + Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route("/prono", get(current::<S>))
        .route("/prono/vote", post(vote::<S>))
        .route("/prono/start", post(start::<S>))
        .route("/leaderboard", get(leaderboard::<S>))
}

async fn current<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(sport): Extension<Arc<Sport>>,
    AuthUser(user): AuthUser,
) -> Result<Response, Response> {
    viewer(&host, &sport, &user)?;
    let (prono, seq) = work(move || (sport.prono_payload(), sport.sequence())).await?;
    Ok(Json(json!({"prono": prono, "seq": seq})).into_response())
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct VoteBody {
    id: String,
    score: String,
}

async fn vote<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(sport): Extension<Arc<Sport>>,
    AuthUser(user): AuthUser,
    Json(body): Json<VoteBody>,
) -> Result<Response, Response> {
    viewer(&host, &sport, &user)?;
    work(move || sport.vote(&user.id, &user.username, &body.id, &body.score))
        .await?
        .map(|()| Json(json!({"ok": true})).into_response())
        .map_err(|e| refuse(vote_status(e), e.code()))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct StartBody {
    home: String,
    away: String,
}

async fn start<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(sport): Extension<Arc<Sport>>,
    AuthUser(user): AuthUser,
    Json(body): Json<StartBody>,
) -> Result<Response, Response> {
    admin(&host, &sport, &user)?;
    match work(move || sport.start_prono(&body.home, &body.away)).await? {
        Ok(started) => Ok(Json(started).into_response()),
        Err(reason) => Ok((
            StatusCode::CONFLICT,
            Json(json!({"ok": false, "error": reason})),
        )
            .into_response()),
    }
}

async fn leaderboard<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(sport): Extension<Arc<Sport>>,
    AuthUser(user): AuthUser,
) -> Result<Response, Response> {
    viewer(&host, &sport, &user)?;
    Ok(Json(json!({"season": work(move || sport.leaderboard()).await?})).into_response())
}
