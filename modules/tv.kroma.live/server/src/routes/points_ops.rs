use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use kroma_module_sdk::host::HostCtx;
use serde::{Deserialize, Serialize};

use super::points::live_of;
use crate::db::{self, HistoryPoint, ProviderEvent};
use crate::model::ProviderId;
use crate::runtime::{now_ms, ProbeReport, ProviderSwitch, Restart, ScanState};

pub const DEFAULT_HISTORY_POINTS: u32 = 360;
pub const MAX_HISTORY_POINTS: u32 = 1080;

pub fn routes<S: HostCtx + Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route("/_port/tv.kroma.live/control/snapshot", post(snapshot::<S>))
        .route("/_port/tv.kroma.live/control/clipCut", post(clip_cut::<S>))
        .route("/_port/tv.kroma.live/control/clipBufferInfo", post(clip_buffer_info::<S>))
        .route("/_port/tv.kroma.live/control/restart", post(restart::<S>))
        .route("/_port/tv.kroma.live/control/switchProvider", post(switch_provider::<S>))
        .route("/_port/tv.kroma.live/control/probe", post(probe::<S>))
        .route("/_port/tv.kroma.live/control/benchmark", post(benchmark::<S>))
        .route("/_port/tv.kroma.live/control/scanResult", post(scan_result::<S>))
        .route("/_port/tv.kroma.live/control/history", post(history::<S>))
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotReply {
    pub jpeg_base64: String,
    pub at: i64,
}

async fn snapshot<S: HostCtx + Clone + Send + Sync + 'static>(State(host): State<S>) -> Json<Result<SnapshotReply, String>> {
    let live = match live_of(&host) {
        Ok(live) => live,
        Err(error) => return Json(Err(error.to_string())),
    };
    Json(live.grab_frame(false).await.map(|frame| SnapshotReply { jpeg_base64: STANDARD.encode(&frame.jpeg), at: frame.at }).map_err(|e| e.as_str().to_string()))
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct ClipCutCall {
    from_ms: i64,
    to_ms: i64,
    name: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClipCutReply {
    pub path: String,
    pub duration_ms: i64,
}

async fn clip_cut<S: HostCtx + Clone + Send + Sync + 'static>(State(host): State<S>, Json(call): Json<ClipCutCall>) -> Json<Result<ClipCutReply, String>> {
    let live = match live_of(&host) {
        Ok(live) => live,
        Err(error) => return Json(Err(error.to_string())),
    };
    Json(live.clip_cut(call.from_ms, call.to_ms, &call.name).await.map(|cut| ClipCutReply { path: cut.path, duration_ms: cut.duration_ms }).map_err(|e| e.as_str().to_string()))
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClipBufferReply {
    pub from_ms: Option<i64>,
    pub to_ms: Option<i64>,
}

async fn clip_buffer_info<S: HostCtx + Clone + Send + Sync + 'static>(State(host): State<S>) -> Json<Result<ClipBufferReply, String>> {
    Json(live_of(&host).map(|live| {
        let bounds = live.clip_buffer_info();
        ClipBufferReply { from_ms: bounds.map(|b| b.0), to_ms: bounds.map(|b| b.1) }
    }).map_err(|e| e.to_string()))
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct RestartCall {
    requested_by: String,
    reason: String,
}

async fn restart<S: HostCtx + Clone + Send + Sync + 'static>(State(host): State<S>, Json(call): Json<RestartCall>) -> Json<Result<Restart, String>> {
    let live = match live_of(&host) {
        Ok(live) => live,
        Err(error) => return Json(Err(error.to_string())),
    };
    Json(Ok(live.restart(&call.requested_by, &call.reason).await))
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct SwitchProviderCall {
    provider: String,
    requested_by: String,
    reason: String,
}

async fn switch_provider<S: HostCtx + Clone + Send + Sync + 'static>(State(host): State<S>, Json(call): Json<SwitchProviderCall>) -> Json<Result<ProviderSwitch, String>> {
    let live = match live_of(&host) {
        Ok(live) => live,
        Err(error) => return Json(Err(error.to_string())),
    };
    let Some(target) = ProviderId::parse(&call.provider) else {
        return Json(Ok(ProviderSwitch { switched: false, provider: call.provider, error: Some("unknownProvider".into()) }));
    };
    Json(Ok(live.switch_provider(target, &call.requested_by, &call.reason).await))
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct ProbeCall {
    channel_id: String,
}

async fn probe<S: HostCtx + Clone + Send + Sync + 'static>(State(host): State<S>, Json(call): Json<ProbeCall>) -> Json<Result<ProbeReport, String>> {
    let live = match live_of(&host) {
        Ok(live) => live,
        Err(error) => return Json(Err(error.to_string())),
    };
    Json(Ok(live.probe_channel(&call.channel_id).await))
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct BenchmarkCall {
    channel_ids: Vec<String>,
    requested_by: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkReply {
    pub status: String,
    pub id: Option<String>,
    pub reason: Option<String>,
    pub total: usize,
    pub already_done: bool,
}

async fn benchmark<S: HostCtx + Clone + Send + Sync + 'static>(State(host): State<S>, Json(call): Json<BenchmarkCall>) -> Json<Result<BenchmarkReply, String>> {
    let live = match live_of(&host) {
        Ok(live) => live,
        Err(error) => return Json(Err(error.to_string())),
    };
    let total = call.channel_ids.len().min(crate::runtime::BENCHMARK_MAX_IDS);
    tracing::info!(requested_by = %call.requested_by, total, "benchmark requested");
    Json(Ok(match live.start_benchmark(&call.channel_ids) {
        Ok((id, already_done)) => BenchmarkReply { status: "started".into(), id: Some(id), reason: None, total, already_done },
        Err(reason) => BenchmarkReply { status: "refused".into(), id: None, reason: Some(reason), total, already_done: false },
    }))
}

async fn scan_result<S: HostCtx + Clone + Send + Sync + 'static>(State(host): State<S>) -> Json<Result<ScanState, String>> {
    Json(live_of(&host).map(|live| live.scan_state()).map_err(|e| e.to_string()))
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct HistoryCall {
    limit: u32,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryReply {
    pub step_seconds: i64,
    pub points: Vec<HistoryPoint>,
    pub provider_events: Vec<ProviderEvent>,
}

async fn history<S: HostCtx + Clone + Send + Sync + 'static>(State(host): State<S>, Json(call): Json<HistoryCall>) -> Json<Result<HistoryReply, String>> {
    let limit = if call.limit == 0 { DEFAULT_HISTORY_POINTS } else { call.limit.min(MAX_HISTORY_POINTS) } as usize;
    Json(live_of(&host).map(|live| {
        let mut points = db::history_since(live.pool(), db::MAX_MINUTES, now_ms() / 1000).unwrap_or_default();
        let skip = points.len().saturating_sub(limit);
        points.drain(..skip);
        HistoryReply { step_seconds: db::STEP_SECONDS, points, provider_events: db::provider_events(live.pool()).unwrap_or_default() }
    }).map_err(|e| e.to_string()))
}
