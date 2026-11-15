use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use kroma_module_sdk::host::{port_reply, HostCtx};
use serde::{Deserialize, Serialize};

use crate::model::Programme;
use crate::presence::Viewer;
use crate::runtime::{now_ms, Live, SwitchRequest};
use crate::status::{ChannelRef, HealthView, Phase, Slot};
use crate::model::ProviderId;

pub fn point_routes<S: HostCtx + Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route("/_port/tv.kroma.live/control/status", post(status::<S>))
        .route("/_port/tv.kroma.live/control/resolve", post(resolve::<S>))
        .route("/_port/tv.kroma.live/control/switch", post(switch::<S>))
        .route("/_port/tv.kroma.live/control/presence", post(presence::<S>))
        .route("/_port/tv.kroma.live/control/proposal", post(proposal::<S>))
        .route("/_port/tv.kroma.live/control/epg", post(epg::<S>))
        .merge(super::points_ops::routes::<S>())
}

pub(super) fn live_of<S: HostCtx>(host: &S) -> anyhow::Result<std::sync::Arc<Live>> {
    kroma_module_sdk::host::service::<Live>(host).ok_or_else(|| anyhow::anyhow!("live is not running"))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct StatusReply {
    pub state: Phase,
    pub channel: Option<ChannelRef>,
    pub provider: Option<ProviderId>,
    pub now: Option<Slot>,
    pub next: Option<Slot>,
    pub viewers: Vec<ViewerName>,
    pub health: PointHealth,
    pub started_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ViewerName {
    pub user_id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PointHealth {
    pub source_kbps: f64,
    pub stutter: bool,
    pub black: bool,
    pub frozen: bool,
}

pub fn status_reply(live: &Live) -> StatusReply {
    let status = live.status();
    let HealthView { source_kbps, stutter, black, frozen, .. } = status.health;
    StatusReply {
        state: status.state,
        channel: status.channel,
        provider: Some(status.provider),
        now: status.now,
        next: status.next,
        viewers: status.viewers.into_iter().map(|v| ViewerName { user_id: v.user_id, name: v.name }).collect(),
        health: PointHealth { source_kbps, stutter, black, frozen },
        started_at: status.started_at,
    }
}

async fn status<S: HostCtx + Clone + Send + Sync + 'static>(State(host): State<S>) -> Json<Result<StatusReply, String>> {
    Json(live_of(&host).map(|live| status_reply(&live)).map_err(|e| e.to_string()))
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ResolveCall {
    query: String,
    limit: u32,
    provider: Option<ProviderId>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ResolvedChannel {
    pub id: String,
    pub name: String,
    pub group: String,
    pub score: f64,
    pub now: Option<Slot>,
    pub provider: Option<ProviderId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<crate::norm::EventInfo>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct ResolveReply {
    pub channels: Vec<ResolvedChannel>,
}

async fn resolve<S: HostCtx + Clone + Send + Sync + 'static>(State(host): State<S>, Json(call): Json<ResolveCall>) -> Json<Result<ResolveReply, String>> {
    let limit = if call.limit == 0 { 5 } else { call.limit as usize };
    Json(live_of(&host).map(|live| ResolveReply {
        channels: live.resolve_in(&call.query, limit, call.provider).into_iter().map(|c| ResolvedChannel { id: c.id, name: c.name, group: c.group, score: c.score, now: c.now, provider: c.provider, event: c.event }).collect(),
    }).map_err(|e| e.to_string()))
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct SwitchCall {
    channel_id: String,
    requested_by: String,
    reason: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct SwitchReply {
    pub switched: bool,
    pub channel: Option<ChannelRef>,
    pub error: Option<String>,
}

async fn switch<S: HostCtx + Clone + Send + Sync + 'static>(State(host): State<S>, Json(call): Json<SwitchCall>) -> Json<Result<SwitchReply, String>> {
    let live = match live_of(&host) {
        Ok(live) => live,
        Err(error) => return Json(Err(error.to_string())),
    };
    let channel_id = if crate::model::parse_ref(&call.channel_id).is_some() { call.channel_id.clone() } else { live.resolve(&call.channel_id, 1).into_iter().next().map(|c| c.id).unwrap_or_default() };
    let outcome = live.switch(SwitchRequest { channel_id, requested_by: Some(call.requested_by), reason: call.reason, skip_if_same: true, ..SwitchRequest::default() }).await;
    let error = (outcome.status == "error" || outcome.status == "offair").then(|| outcome.error.clone().unwrap_or_else(|| outcome.status.clone()));
    Json(Ok(SwitchReply { switched: outcome.switched(), channel: outcome.channel, error }))
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct PresenceReply {
    pub viewers: Vec<PresenceViewer>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PresenceViewer {
    pub user_id: String,
    pub name: String,
    pub since: i64,
}

async fn presence<S: HostCtx + Clone + Send + Sync + 'static>(State(host): State<S>) -> Json<Result<PresenceReply, String>> {
    Json(live_of(&host).map(|live| PresenceReply {
        viewers: live.lock().presence.viewers().into_iter().map(|Viewer { user_id, name, since }| PresenceViewer { user_id, name, since }).collect(),
    }).map_err(|e| e.to_string()))
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ProposalCall {
    id: String,
    accept: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct ProposalReply {
    pub applied: bool,
}

async fn proposal<S: HostCtx + Clone + Send + Sync + 'static>(State(host): State<S>, Json(call): Json<ProposalCall>) -> Json<Result<ProposalReply, String>> {
    let live = match live_of(&host) {
        Ok(live) => live,
        Err(error) => return Json(Err(error.to_string())),
    };
    Json(Ok(ProposalReply { applied: live.answer_proposal(&call.id, call.accept).await }))
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct EpgCall {
    query: Option<String>,
    from: Option<i64>,
    to: Option<i64>,
    limit: u32,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct EpgReply {
    pub programmes: Vec<Programme>,
}

async fn epg<S: HostCtx + Clone + Send + Sync + 'static>(State(host): State<S>, Json(call): Json<EpgCall>) -> Json<Result<EpgReply, String>> {
    let limit = if call.limit == 0 { 20 } else { call.limit as usize };
    let _ = now_ms();
    let reply = port_reply(move || {
        let live = live_of(&host)?;
        Ok(EpgReply { programmes: live.guide(call.query.as_deref(), call.from, call.to, limit) })
    })
    .await;
    reply
}
