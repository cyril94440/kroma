use std::sync::Arc;

use axum::extract::{Extension, Query, State};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use kroma_module_sdk::host::{AuthUser, HostCtx};

use super::{viewer, work};
use crate::runtime::Sport;

pub fn routes<S: HostCtx + Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new().route("/standings", get(standings::<S>))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct StandingsQuery {
    competition: String,
}

async fn standings<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(sport): Extension<Arc<Sport>>,
    AuthUser(user): AuthUser,
    Query(q): Query<StandingsQuery>,
) -> Result<Response, Response> {
    viewer(&host, &user)?;
    let cached = work(move || sport.standings_for_page(&q.competition)).await?;
    Ok(Json(json!({"standings": cached, "pending": cached.is_none()})).into_response())
}
