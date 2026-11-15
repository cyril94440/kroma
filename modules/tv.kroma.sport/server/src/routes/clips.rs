use std::sync::Arc;

use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use kroma_module_sdk::host::{AuthUser, HostCtx};

use super::{admin, refuse, viewer, work};
use crate::runtime::Sport;

pub fn routes<S: HostCtx + Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route("/clips", get(list::<S>))
        .route("/clips/{id}", delete(remove::<S>))
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ListQuery {
    match_id: Option<String>,
    limit: Option<u32>,
}

async fn list<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(sport): Extension<Arc<Sport>>,
    AuthUser(user): AuthUser,
    Query(q): Query<ListQuery>,
) -> Result<Response, Response> {
    viewer(&host, &user)?;
    let clips = work(move || sport.clips(q.match_id.as_deref(), q.limit.unwrap_or(100))).await?;
    Ok(Json(json!({"clips": clips})).into_response())
}

async fn remove<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(sport): Extension<Arc<Sport>>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> Result<Response, Response> {
    admin(&host, &user)?;
    if work(move || sport.delete_clip(&id)).await? {
        Ok(Json(json!({"ok": true})).into_response())
    } else {
        Err(refuse(StatusCode::NOT_FOUND, "notFound"))
    }
}
