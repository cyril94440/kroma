use std::sync::Arc;

use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use kroma_module_sdk::host::{AuthUser, HostCtx};

use super::{actor, admin, refuse, work};
use crate::party::{IdeaError, IdeaInput, Party};

pub fn routes<S>() -> Router<S>
where
    S: HostCtx + Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/ideas", get(list::<S>).post(submit::<S>))
        .route("/ideas/triage", get(triage::<S>))
        .route("/ideas/{id}", post(update::<S>))
}

async fn list<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
) -> Result<Response, Response> {
    actor(&host, &party, &user)?;
    let ideas = work(move || party.ideas_view()).await?;
    Ok(Json(json!({ "ideas": ideas })).into_response())
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct SubmitBody {
    genre: String,
    text: String,
    channel: Option<String>,
    device: Option<String>,
    version: Option<String>,
    browser: Option<String>,
}

fn status_of(e: &IdeaError) -> StatusCode {
    match e {
        IdeaError::TooFast | IdeaError::TooMany => StatusCode::TOO_MANY_REQUESTS,
        IdeaError::Unknown => StatusCode::NOT_FOUND,
        IdeaError::Storage => StatusCode::INTERNAL_SERVER_ERROR,
        IdeaError::Empty | IdeaError::BadStatus => StatusCode::BAD_REQUEST,
    }
}

async fn submit<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
    Json(body): Json<SubmitBody>,
) -> Result<Response, Response> {
    let who = actor(&host, &party, &user)?;
    let input = IdeaInput {
        genre: body.genre,
        text: body.text,
        channel: body.channel,
        device: body.device,
        version: body.version,
        browser: body.browser,
        source: None,
    };
    work(move || party.submit_idea(&who.person, input, false))
        .await?
        .map(|id| Json(json!({ "id": id })).into_response())
        .map_err(|e| refuse(status_of(&e), e.code()))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct TriageQuery {
    status: Option<String>,
}

async fn triage<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
    Query(q): Query<TriageQuery>,
) -> Result<Response, Response> {
    admin(&host, &party, &user)?;
    let ideas = work(move || party.ideas_for_triage(q.status.as_deref())).await?;
    Ok(Json(json!({ "ideas": ideas })).into_response())
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct UpdateBody {
    status: Option<String>,
    reply: Option<String>,
}

async fn update<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
    Json(body): Json<UpdateBody>,
) -> Result<Response, Response> {
    admin(&host, &party, &user)?;
    work(move || party.update_idea(&id, body.status.as_deref(), body.reply.as_deref()))
        .await?
        .map(|idea| Json(json!({ "idea": idea })).into_response())
        .map_err(|e| refuse(status_of(&e), e.code()))
}
