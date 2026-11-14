use std::sync::Arc;

use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

use kroma_module_sdk::domain::Permission;
use kroma_module_sdk::host::{json_error, run_blocking, AuthUser, HostCtx};

use crate::agent::Agent;
use crate::rules::DEFAULT_RULES;

pub fn routes<S>(agent: Arc<Agent>) -> Router<S>
where
    S: HostCtx + Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/status", get(status::<S>))
        .route("/pause", post(pause::<S>))
        .route("/audit", get(audit::<S>))
        .route("/rules", get(rules::<S>).put(save_rules::<S>))
        .route("/rules/reset", post(reset_rules::<S>))
        .route("/lessons", get(lessons::<S>))
        .route("/lessons/{id}", delete(delete_lesson::<S>))
        .route("/suggestions", get(suggestions::<S>))
        .route("/suggestions/{id}", delete(delete_suggestion::<S>))
        .route("/notes", get(notes::<S>))
        .route("/notes/{user_id}", delete(forget_notes::<S>))
        .route("/model/test", post(test_model::<S>))
        .route("/model/list", get(list_models::<S>))
        .layer(Extension(agent))
}

fn admin<S: HostCtx>(host: &S, user: &kroma_module_sdk::domain::User) -> Result<(), Response> {
    if !user.can(Permission::SettingsManage) {
        return Err(json_error(StatusCode::FORBIDDEN, "forbidden"));
    }
    host.require(user, Permission::SettingsManage)
}

async fn work<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, Response> {
    run_blocking(f).await.map_err(|_| json_error(StatusCode::INTERNAL_SERVER_ERROR, "internal"))
}

fn internal(e: anyhow::Error) -> Response {
    tracing::error!(error = %e, "hermes route failed");
    json_error(StatusCode::INTERNAL_SERVER_ERROR, "internal")
}

async fn status<S: HostCtx + Clone>(State(host): State<S>, Extension(agent): Extension<Arc<Agent>>, AuthUser(user): AuthUser) -> Result<Response, Response> {
    admin(&host, &user)?;
    Ok(Json(work(move || agent.status_json()).await?).into_response())
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct PauseBody {
    paused: bool,
}

async fn pause<S: HostCtx + Clone>(State(host): State<S>, Extension(agent): Extension<Arc<Agent>>, AuthUser(user): AuthUser, Json(body): Json<PauseBody>) -> Result<Response, Response> {
    admin(&host, &user)?;
    work(move || agent.set_paused(body.paused)).await?;
    Ok(Json(json!({ "ok": true })).into_response())
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct AuditQuery {
    genre: Option<String>,
    before: Option<i64>,
    limit: Option<u32>,
}

async fn audit<S: HostCtx + Clone>(State(host): State<S>, Extension(agent): Extension<Arc<Agent>>, AuthUser(user): AuthUser, Query(q): Query<AuditQuery>) -> Result<Response, Response> {
    admin(&host, &user)?;
    let out = work(move || {
        let store = &agent.deps.store;
        let rows = store.audit_rows(q.genre.as_deref().filter(|g| !g.is_empty()), q.before, q.limit.unwrap_or(100))?;
        anyhow::Ok(json!({ "rows": rows, "genres": store.audit_genres()? }))
    })
    .await?
    .map_err(internal)?;
    Ok(Json(out).into_response())
}

async fn rules<S: HostCtx + Clone>(State(host): State<S>, Extension(agent): Extension<Arc<Agent>>, AuthUser(user): AuthUser) -> Result<Response, Response> {
    admin(&host, &user)?;
    let out = work(move || json!({ "text": agent.deps.rules.text(), "default": DEFAULT_RULES, "customised": agent.deps.rules.is_customised() })).await?;
    Ok(Json(out).into_response())
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RulesBody {
    text: String,
}

async fn save_rules<S: HostCtx + Clone>(State(host): State<S>, Extension(agent): Extension<Arc<Agent>>, AuthUser(user): AuthUser, Json(body): Json<RulesBody>) -> Result<Response, Response> {
    admin(&host, &user)?;
    work(move || {
        agent.deps.audit("audit", "rules_saved", json!({ "chars": body.text.chars().count(), "by": user.id }));
        agent.deps.rules.save(&body.text)
    })
    .await?
    .map_err(|e| json_error(StatusCode::BAD_REQUEST, &e.to_string()))?;
    Ok(Json(json!({ "ok": true })).into_response())
}

async fn reset_rules<S: HostCtx + Clone>(State(host): State<S>, Extension(agent): Extension<Arc<Agent>>, AuthUser(user): AuthUser) -> Result<Response, Response> {
    admin(&host, &user)?;
    work(move || agent.deps.rules.reset()).await?.map_err(internal)?;
    Ok(Json(json!({ "ok": true })).into_response())
}

async fn lessons<S: HostCtx + Clone>(State(host): State<S>, Extension(agent): Extension<Arc<Agent>>, AuthUser(user): AuthUser) -> Result<Response, Response> {
    admin(&host, &user)?;
    let rows = work(move || agent.deps.store.lessons()).await?.map_err(internal)?;
    Ok(Json(json!({ "lessons": rows })).into_response())
}

async fn delete_lesson<S: HostCtx + Clone>(State(host): State<S>, Extension(agent): Extension<Arc<Agent>>, AuthUser(user): AuthUser, Path(id): Path<i64>) -> Result<Response, Response> {
    admin(&host, &user)?;
    let removed = work(move || agent.deps.store.delete_lesson(id)).await?.map_err(internal)?;
    Ok(Json(json!({ "ok": removed })).into_response())
}

async fn suggestions<S: HostCtx + Clone>(State(host): State<S>, Extension(agent): Extension<Arc<Agent>>, AuthUser(user): AuthUser) -> Result<Response, Response> {
    admin(&host, &user)?;
    let rows = work(move || agent.deps.store.suggestions()).await?.map_err(internal)?;
    Ok(Json(json!({ "suggestions": rows })).into_response())
}

async fn delete_suggestion<S: HostCtx + Clone>(State(host): State<S>, Extension(agent): Extension<Arc<Agent>>, AuthUser(user): AuthUser, Path(id): Path<i64>) -> Result<Response, Response> {
    admin(&host, &user)?;
    let removed = work(move || agent.deps.store.delete_suggestion(id)).await?.map_err(internal)?;
    Ok(Json(json!({ "ok": removed })).into_response())
}

async fn notes<S: HostCtx + Clone>(State(host): State<S>, Extension(agent): Extension<Arc<Agent>>, AuthUser(user): AuthUser) -> Result<Response, Response> {
    admin(&host, &user)?;
    let rows = work(move || agent.deps.store.all_notes()).await?.map_err(internal)?;
    let notes: Vec<Value> = rows.into_iter().map(|(user_id, name, note)| json!({ "userId": user_id, "name": name, "note": note })).collect();
    Ok(Json(json!({ "notes": notes })).into_response())
}

async fn forget_notes<S: HostCtx + Clone>(State(host): State<S>, Extension(agent): Extension<Arc<Agent>>, AuthUser(user): AuthUser, Path(user_id): Path<String>) -> Result<Response, Response> {
    admin(&host, &user)?;
    let removed = work(move || agent.deps.store.forget_person(&user_id)).await?.map_err(internal)?;
    Ok(Json(json!({ "removed": removed })).into_response())
}

fn model_failure(agent: &Agent, error: &dyn std::fmt::Display) -> Response {
    let text = crate::blackbox::mask(&error.to_string(), &agent.deps.settings().secrets());
    (StatusCode::OK, Json(json!({ "ok": false, "error": text }))).into_response()
}

async fn test_model<S: HostCtx + Clone>(State(host): State<S>, Extension(agent): Extension<Arc<Agent>>, AuthUser(user): AuthUser) -> Result<Response, Response> {
    admin(&host, &user)?;
    let reply = work(move || {
        let settings = agent.deps.settings();
        let Some(model) = agent.deps.model() else {
            return model_failure(&agent, &crate::model::ModelError::NotConfigured);
        };
        let started = agent.deps.now();
        match model.ping() {
            Ok(text) => {
                agent.deps.audit("llm", "test", json!({ "ok": true, "ms": agent.deps.now() - started }));
                Json(json!({ "ok": true, "provider": format!("{:?}", settings.provider).to_lowercase(), "model": settings.active_model(), "answer": text, "ms": agent.deps.now() - started, "search": model.can_search() })).into_response()
            }
            Err(e) => {
                agent.deps.audit("llm", "test", json!({ "ok": false, "error": e.to_string() }));
                model_failure(&agent, &e)
            }
        }
    })
    .await?;
    Ok(reply)
}

async fn list_models<S: HostCtx + Clone>(State(host): State<S>, Extension(agent): Extension<Arc<Agent>>, AuthUser(user): AuthUser) -> Result<Response, Response> {
    admin(&host, &user)?;
    let reply = work(move || {
        let Some(model) = agent.deps.model() else {
            return model_failure(&agent, &crate::model::ModelError::NotConfigured);
        };
        match model.models() {
            Ok(models) => Json(json!({ "ok": true, "models": models })).into_response(),
            Err(e) => model_failure(&agent, &e),
        }
    })
    .await?;
    Ok(reply)
}

#[cfg(test)]
mod tests;
