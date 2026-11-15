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

use crate::runtime::{CalibrationError, Sport, VoteError};

mod clips;
mod prono;
mod standings;

pub fn routes<S>(sport: Arc<Sport>) -> Router<S>
where
    S: HostCtx + Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/state", get(state::<S>))
        .route("/view", get(view::<S>))
        .route("/open", post(open::<S>))
        .route("/watch", post(watch::<S>))
        .route("/match", get(current_match::<S>))
        .route("/calibration", post(calibration::<S>))
        .route("/trace", get(trace::<S>))
        .route("/refresh", post(refresh::<S>))
        .merge(prono::routes::<S>())
        .merge(standings::routes::<S>())
        .merge(clips::routes::<S>())
        .layer(Extension(sport))
}

pub(crate) fn is_admin(user: &User) -> bool {
    user.can(Permission::SettingsManage) || user.can(Permission::UsersManage)
}

fn require<S: HostCtx>(host: &S, user: &User, permission: Permission) -> Result<(), Response> {
    if !user.can(permission) {
        return Err(json_error(StatusCode::FORBIDDEN, "forbidden"));
    }
    host.require(user, permission)
}

pub(crate) fn viewer<S: HostCtx>(host: &S, sport: &Sport, user: &User) -> Result<(), Response> {
    require(host, user, Permission::Playback)?;
    sport.note_role(&user.id, user.can(Permission::SettingsManage));
    Ok(())
}

pub(crate) fn admin<S: HostCtx>(host: &S, sport: &Sport, user: &User) -> Result<(), Response> {
    viewer(host, sport, user)?;
    require(host, user, Permission::SettingsManage)
}

pub(crate) async fn work<T: Send + 'static>(
    f: impl FnOnce() -> T + Send + 'static,
) -> Result<T, Response> {
    run_blocking(f)
        .await
        .map_err(|_| json_error(StatusCode::INTERNAL_SERVER_ERROR, "internal"))
}

pub(crate) fn refuse(status: StatusCode, code: &str) -> Response {
    json_error(status, code)
}

async fn state<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(sport): Extension<Arc<Sport>>,
    AuthUser(user): AuthUser,
) -> Result<Response, Response> {
    viewer(&host, &sport, &user)?;
    let admin = is_admin(&user);
    Ok(Json(work(move || sport.snapshot(&user.id, &user.username, admin)).await?).into_response())
}

async fn view<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(sport): Extension<Arc<Sport>>,
    AuthUser(user): AuthUser,
) -> Result<Response, Response> {
    viewer(&host, &sport, &user)?;
    let snapshot = work(move || sport.snapshot(&user.id, &user.username, false)).await?;
    Ok(Json(snapshot["view"].clone()).into_response())
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct OpenBody {
    force: bool,
}

async fn open<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(sport): Extension<Arc<Sport>>,
    AuthUser(user): AuthUser,
    Json(body): Json<OpenBody>,
) -> Result<Response, Response> {
    viewer(&host, &sport, &user)?;
    let accepted = work(move || sport.open_view(&user.id, body.force)).await?;
    Ok(Json(json!({"ok": true, "accepted": accepted})).into_response())
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct WatchBody {
    channel_id: String,
}

async fn watch<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(sport): Extension<Arc<Sport>>,
    AuthUser(user): AuthUser,
    Json(body): Json<WatchBody>,
) -> Result<Response, Response> {
    viewer(&host, &sport, &user)?;
    if body.channel_id.is_empty() || body.channel_id.len() > 80 {
        return Err(refuse(StatusCode::BAD_REQUEST, "invalidChannel"));
    }
    match work(move || sport.watch(&user.id, &body.channel_id)).await? {
        Ok(outcome) => Ok(Json(json!({"outcome": outcome.outcome, "voteId": outcome.vote_id, "reason": outcome.reason})).into_response()),
        Err(_) => Err(refuse(StatusCode::SERVICE_UNAVAILABLE, "roomUnavailable")),
    }
}

async fn current_match<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(sport): Extension<Arc<Sport>>,
    AuthUser(user): AuthUser,
) -> Result<Response, Response> {
    viewer(&host, &sport, &user)?;
    let snapshot = work(move || sport.snapshot(&user.id, &user.username, false)).await?;
    Ok(Json(json!({"match": snapshot["match"], "seq": snapshot["seq"]})).into_response())
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct CalibrationBody {
    clock: String,
    captured_at_ms: Option<i64>,
}

async fn calibration<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(sport): Extension<Arc<Sport>>,
    AuthUser(user): AuthUser,
    Json(body): Json<CalibrationBody>,
) -> Result<Response, Response> {
    viewer(&host, &sport, &user)?;
    match work(move || sport.calibrate(&body.clock, body.captured_at_ms)).await? {
        Ok(done) => Ok(Json(done).into_response()),
        Err(e) => Err(refuse(
            if e == CalibrationError::Unreadable {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::CONFLICT
            },
            e.code(),
        )),
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct TraceQuery {
    genre: Option<String>,
    before: Option<i64>,
    limit: Option<u32>,
}

async fn trace<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(sport): Extension<Arc<Sport>>,
    AuthUser(user): AuthUser,
    Query(q): Query<TraceQuery>,
) -> Result<Response, Response> {
    admin(&host, &sport, &user)?;
    let rows = work(move || {
        sport.trace_rows(
            q.genre.as_deref(),
            q.before,
            q.limit.unwrap_or(100).min(500),
        )
    })
    .await?;
    Ok(Json(json!({"rows": rows})).into_response())
}

async fn refresh<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(sport): Extension<Arc<Sport>>,
    AuthUser(user): AuthUser,
) -> Result<Response, Response> {
    admin(&host, &sport, &user)?;
    work(move || sport.refresh()).await?;
    Ok(Json(json!({"ok": true})).into_response())
}

pub(crate) fn vote_status(error: VoteError) -> StatusCode {
    match error {
        VoteError::NoProno => StatusCode::NOT_FOUND,
        VoteError::Closed => StatusCode::CONFLICT,
        VoteError::Invalid => StatusCode::BAD_REQUEST,
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests;
