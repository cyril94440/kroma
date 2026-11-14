use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use kroma_module_sdk::host::{json_error, AuthUser, HostCtx};
use serde::{Deserialize, Serialize};

use super::{admin_only, live};
use crate::db::{self, HistoryPoint, ProviderEvent};
use crate::model::ProviderId;
use crate::runtime::{now_ms, ScanState, SwitchOutcome};
use crate::trial::TrialSummary;

pub fn routes<S: HostCtx + Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route("/pause", post(pause::<S>))
        .route("/resume", post(resume::<S>))
        .route("/scan", post(scan_start::<S>).get(scan_state::<S>))
        .route("/trial", post(trial::<S>))
        .route("/history", get(history::<S>))
        .route("/metrics", get(metrics::<S>))
        .route("/overview", get(overview::<S>))
        .route("/refresh", post(refresh::<S>))
}

async fn pause<S: HostCtx + Clone + Send + Sync + 'static>(State(host): State<S>, AuthUser(user): AuthUser) -> Result<Json<serde_json::Value>, Response> {
    admin_only(&host, &user)?;
    Ok(Json(serde_json::json!({ "status": live(&host)?.pause().await })))
}

async fn resume<S: HostCtx + Clone + Send + Sync + 'static>(State(host): State<S>, AuthUser(user): AuthUser) -> Result<Json<SwitchOutcome>, Response> {
    admin_only(&host, &user)?;
    Ok(Json(live(&host)?.resume().await))
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct ScanBody {
    query: String,
    provider: Option<ProviderId>,
}

async fn scan_start<S: HostCtx + Clone + Send + Sync + 'static>(State(host): State<S>, AuthUser(user): AuthUser, Json(body): Json<ScanBody>) -> Result<Json<serde_json::Value>, Response> {
    admin_only(&host, &user)?;
    let id = live(&host)?.start_scan(&body.query, body.provider.unwrap_or(ProviderId::Primary)).map_err(|e| json_error(StatusCode::CONFLICT, &e))?;
    Ok(Json(serde_json::json!({ "id": id })))
}

async fn scan_state<S: HostCtx + Clone + Send + Sync + 'static>(State(host): State<S>, AuthUser(user): AuthUser) -> Result<Json<ScanState>, Response> {
    admin_only(&host, &user)?;
    Ok(Json(live(&host)?.scan_state()))
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct TrialBody {
    channel_id: String,
    seconds: u64,
}

async fn trial<S: HostCtx + Clone + Send + Sync + 'static>(State(host): State<S>, AuthUser(user): AuthUser, Json(body): Json<TrialBody>) -> Result<Json<TrialSummary>, Response> {
    admin_only(&host, &user)?;
    let live = live(&host)?;
    let target = live.providers.find_by_ref(&body.channel_id).ok_or_else(|| json_error(StatusCode::NOT_FOUND, "unknownChannel"))?;
    live.trial(&target, body.seconds).await.map(Json).map_err(|e| json_error(StatusCode::CONFLICT, &e))
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct HistoryParams {
    minutes: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HistoryAnswer {
    step_seconds: i64,
    points: Vec<HistoryPoint>,
    provider_events: Vec<ProviderEvent>,
}

async fn history<S: HostCtx + Clone + Send + Sync + 'static>(State(host): State<S>, AuthUser(user): AuthUser, Query(params): Query<HistoryParams>) -> Result<Json<HistoryAnswer>, Response> {
    admin_only(&host, &user)?;
    let live = live(&host)?;
    let points = db::history_since(live.pool(), params.minutes.unwrap_or(60), now_ms() / 1000).unwrap_or_default();
    Ok(Json(HistoryAnswer { step_seconds: db::STEP_SECONDS, points, provider_events: db::provider_events(live.pool()).unwrap_or_default() }))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MetricsAnswer {
    zaps: usize,
    last_ms: Option<i64>,
    p50_ms: Option<i64>,
    p95_ms: Option<i64>,
    last_phases: Option<db::ZapRecord>,
    restarts_total: u64,
}

async fn metrics<S: HostCtx + Clone + Send + Sync + 'static>(State(host): State<S>, AuthUser(user): AuthUser) -> Result<Json<MetricsAnswer>, Response> {
    admin_only(&host, &user)?;
    let live = live(&host)?;
    let zaps = db::recent_zaps(live.pool()).unwrap_or_default();
    let totals: Vec<i64> = zaps.iter().map(|z| z.total_ms).collect();
    let restarts_total = live.lock().restarts_total;
    Ok(Json(MetricsAnswer { zaps: zaps.len(), last_ms: totals.last().copied(), p50_ms: db::percentile(&totals, 50.0), p95_ms: db::percentile(&totals, 95.0), last_phases: zaps.last().cloned(), restarts_total }))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceView {
    provider: ProviderId,
    configured: bool,
    channels: i64,
    fetched_at: i64,
    error: Option<String>,
    single_connection: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OverviewAnswer {
    providers: Vec<SourceView>,
    guide_programmes: i64,
    guide_fetched_at: i64,
    guide_source: String,
    guide_error: Option<String>,
    ffmpeg_https: bool,
    favourites: Vec<String>,
    standby_seconds: u64,
}

async fn overview<S: HostCtx + Clone + Send + Sync + 'static>(State(host): State<S>, AuthUser(user): AuthUser) -> Result<Json<OverviewAnswer>, Response> {
    admin_only(&host, &user)?;
    let live = live(&host)?;
    let (settings, rules) = (live.settings(), live.rules());
    let providers = [ProviderId::Primary, ProviderId::Backup]
        .into_iter()
        .map(|provider| {
            let state = db::catalogue_state(live.pool(), provider).unwrap_or_default();
            SourceView { provider, configured: !settings.url_of(provider).is_empty(), channels: state.channels, fetched_at: state.fetched_at, error: state.error, single_connection: settings.single_connection().contains(&provider) }
        })
        .collect();
    let guide = db::epg_state(live.pool()).unwrap_or_default();
    let ffmpeg_https = live.ffmpeg_supports_https();
    Ok(Json(OverviewAnswer { providers, guide_programmes: guide.programmes, guide_fetched_at: guide.fetched_at, guide_source: guide.source, guide_error: guide.error, ffmpeg_https, favourites: rules.favourites, standby_seconds: settings.standby_seconds }))
}

async fn refresh<S: HostCtx + Clone + Send + Sync + 'static>(State(host): State<S>, AuthUser(user): AuthUser) -> Result<Json<serde_json::Value>, Response> {
    admin_only(&host, &user)?;
    let live = live(&host)?;
    let catalogues: Vec<serde_json::Value> = live
        .refresh_catalogues()
        .into_iter()
        .map(|(provider, result)| match result {
            Ok(count) => serde_json::json!({ "provider": provider, "channels": count }),
            Err(error) => serde_json::json!({ "provider": provider, "error": error }),
        })
        .collect();
    let guide = live.refresh_epg().map(|c| serde_json::json!({ "programmes": c })).unwrap_or_else(|e| serde_json::json!({ "error": e.to_string() }));
    Ok(Json(serde_json::json!({ "catalogues": catalogues, "guide": guide })))
}
