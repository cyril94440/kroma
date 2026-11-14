use std::sync::Arc;

use axum::extract::{Extension, Query, State};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

use kroma_module_sdk::host::{AuthUser, HostCtx};

use super::{actor, admin, ok, work};
use crate::party::{Party, ViewerStats};
use crate::store::LogEntry;

const LOG_BATCH_MAX: usize = 100;

pub fn routes<S>() -> Router<S>
where
    S: HostCtx + Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/stats", post(stats::<S>))
        .route("/clientlog", get(logs::<S>).post(ingest::<S>))
        .route("/health", get(health::<S>))
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct StatsBody {
    buffer: f64,
    bandwidth_kbps: f64,
    stalls: u32,
    rebuffer_ms: u64,
    dropped: u32,
    resolution: String,
    playing: bool,
    delay: Option<f64>,
    paused: bool,
}

async fn stats<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
    Json(b): Json<StatsBody>,
) -> Result<Response, Response> {
    let who = actor(&host, &user)?;
    let finite = |x: f64| if x.is_finite() { x } else { 0.0 };
    party.record_stats(
        &who,
        ViewerStats {
            buffer: finite(b.buffer),
            bandwidth_kbps: finite(b.bandwidth_kbps),
            stalls: b.stalls,
            rebuffer_ms: b.rebuffer_ms,
            dropped: b.dropped,
            resolution: b.resolution.chars().take(32).collect(),
            playing: b.playing,
            delay: b.delay.filter(|d| d.is_finite()),
            paused: b.paused,
            ..ViewerStats::default()
        },
    );
    Ok(ok())
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct LogBody {
    device: Option<String>,
    events: Vec<LogEvent>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct LogEvent {
    at: Option<i64>,
    category: String,
    event: String,
    data: Option<Value>,
}

async fn ingest<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
    Json(body): Json<LogBody>,
) -> Result<Response, Response> {
    let who = actor(&host, &user)?;
    let clip = |s: &str, n: usize| s.chars().take(n).collect::<String>();
    let now = party.time_now();
    let device = body.device.as_deref().map(|d| clip(d, 40));
    let entries: Vec<LogEntry> = body
        .events
        .into_iter()
        .take(LOG_BATCH_MAX)
        .filter(|e| !e.event.is_empty())
        .map(|e| LogEntry {
            at: e.at.filter(|t| (now - 86_400_000..=now + 60_000).contains(t)).unwrap_or(now),
            user_id: who.person.user_id.clone(),
            name: who.person.name.clone(),
            device: device.clone(),
            category: clip(&e.category, 24),
            event: clip(&e.event, 60),
            data: e.data.filter(|d| d.to_string().len() <= 2000),
        })
        .collect();
    work(move || party.ingest_logs(&entries)).await?;
    Ok(ok())
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct LogQuery {
    minutes: Option<i64>,
    user_id: Option<String>,
    category: Option<String>,
    limit: Option<u32>,
}

async fn logs<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
    Query(q): Query<LogQuery>,
) -> Result<Response, Response> {
    admin(&host, &user)?;
    let rows = work(move || party.read_logs(q.minutes.unwrap_or(30), q.user_id.as_deref(), q.category.as_deref(), q.limit.unwrap_or(120)))
        .await?;
    Ok(Json(json!({ "events": rows })).into_response())
}

async fn health<S: HostCtx + Clone>(
    State(host): State<S>,
    Extension(party): Extension<Arc<Party>>,
    AuthUser(user): AuthUser,
) -> Result<Response, Response> {
    admin(&host, &user)?;
    Ok(Json(party.health()).into_response())
}
