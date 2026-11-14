use std::sync::Arc;

use axum::extract::{Extension, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use kroma_module_sdk::domain::{Permission, User};
use kroma_module_sdk::host::{json_error, run_blocking, AuthUser, HostCtx};

use crate::model::Person;
use crate::party::{Actor, Party};

mod govern;
mod ideas;
mod polls;
mod sync;
mod telemetry;

pub fn routes<S>(party: Arc<Party>) -> Router<S>
where
    S: HostCtx + Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/state", get(state::<S>))
        .route("/time", get(time::<S>))
        .route("/chat", post(post_chat::<S>))
        .route("/chat/history", get(history::<S>))
        .route("/react", post(react::<S>))
        .route("/reaction", post(floating::<S>))
        .merge(govern::routes::<S>())
        .merge(polls::routes::<S>())
        .merge(ideas::routes::<S>())
        .merge(sync::routes::<S>())
        .merge(telemetry::routes::<S>())
        .merge(crate::room::routes::<S>())
        .layer(Extension(party))
}

pub(crate) fn person(user: &User) -> Person {
    Person::new(&user.id, &user.username)
}

pub(crate) fn is_admin(user: &User) -> bool {
    user.can(Permission::SettingsManage) || user.can(Permission::UsersManage)
}

pub(crate) fn actor<S: HostCtx>(state: &S, user: &User) -> Result<Actor, Response> {
    state.require(user, Permission::Playback)?;
    Ok(Actor {
        person: person(user),
        admin: is_admin(user),
    })
}

pub(crate) fn admin<S: HostCtx>(state: &S, user: &User) -> Result<Actor, Response> {
    let who = actor(state, user)?;
    state.require(user, Permission::SettingsManage)?;
    Ok(who)
}

pub(crate) async fn work<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, Response> {
    run_blocking(f)
        .await
        .map_err(|_| json_error(StatusCode::INTERNAL_SERVER_ERROR, "internal"))
}

pub(crate) fn refuse(status: StatusCode, code: &str) -> Response {
    json_error(status, code)
}

pub(crate) fn ok() -> Response {
    Json(json!({ "ok": true })).into_response()
}

async fn state<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
) -> Result<Response, Response> {
    let who = actor(&host, &user)?;
    let view = work(move || party.snapshot(&who)).await?;
    Ok(Json(view).into_response())
}

async fn time<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
) -> Result<Response, Response> {
    actor(&host, &user)?;
    Ok(Json(json!({ "now": party.time_now() })).into_response())
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ChatBody {
    text: String,
}

async fn post_chat<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
    Json(body): Json<ChatBody>,
) -> Result<Response, Response> {
    let who = actor(&host, &user)?;
    match work(move || party.post_message(&who, &body.text)).await? {
        Ok(message) => Ok(Json(json!({ "message": message })).into_response()),
        Err(e) => {
            let status = match e {
                crate::party::ChatError::RateLimited => StatusCode::TOO_MANY_REQUESTS,
                _ => StatusCode::BAD_REQUEST,
            };
            Err(refuse(status, e.code()))
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct HistoryQuery {
    limit: Option<u32>,
    before: Option<i64>,
}

async fn history<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
    Query(q): Query<HistoryQuery>,
) -> Result<Response, Response> {
    actor(&host, &user)?;
    let messages = work(move || party.history(q.limit.unwrap_or(100), q.before)).await?;
    Ok(Json(json!({ "messages": messages })).into_response())
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ReactBody {
    id: String,
    emoji: String,
}

async fn react<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
    Json(body): Json<ReactBody>,
) -> Result<Response, Response> {
    let who = actor(&host, &user)?;
    work(move || party.toggle_reaction(&who, &body.id, &body.emoji))
        .await?
        .map(|_| ok())
        .map_err(|e| refuse(StatusCode::BAD_REQUEST, e.code()))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct FloatingBody {
    emoji: String,
}

async fn floating<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
    Json(body): Json<FloatingBody>,
) -> Result<Response, Response> {
    let who = actor(&host, &user)?;
    work(move || party.floating_reaction(&who, &body.emoji))
        .await?
        .map(|_| ok())
        .map_err(|e| {
            let status = if e == crate::party::ChatError::RateLimited { StatusCode::TOO_MANY_REQUESTS } else { StatusCode::BAD_REQUEST };
            refuse(status, e.code())
        })
}

#[cfg(test)]
mod tests;
