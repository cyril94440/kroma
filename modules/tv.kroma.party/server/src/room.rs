use std::sync::Arc;

use axum::extract::{Extension, State};
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use kroma_module_sdk::host::{port_reply, HostCtx};

use crate::model::{Person, RoomMessage};
use crate::party::{Party, Request};

pub const ROOM: &str = "tv.kroma.party/room";

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct SayRequest {
    pub text: String,
    #[serde(rename = "as")]
    pub speaker: String,
    pub card: Option<Value>,
    pub reply_to: Option<String>,
}

#[derive(Serialize)]
pub struct SayReply {
    pub id: String,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct HistoryRequest {
    pub limit: u32,
    pub before: Option<i64>,
}

#[derive(Serialize)]
pub struct HistoryReply {
    pub messages: Vec<RoomMessage>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct SwitchRequest {
    pub channel_id: String,
    pub channel_name: Option<String>,
    pub requested_by: String,
    pub reason: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchReply {
    pub outcome: &'static str,
    pub vote_id: Option<String>,
    pub reason: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct StartPollRequest {
    pub question: String,
    pub kind: String,
    pub options: Vec<String>,
    pub duration_s: Option<i64>,
}

#[derive(Serialize)]
pub struct PollReply {
    pub poll: Value,
}

#[derive(Serialize)]
pub struct ClosedPollReply {
    pub results: Value,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct VotePollRequest {
    pub user_id: String,
    pub value: String,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct KickRequest {
    pub user_id: String,
    pub requested_by: String,
    pub reason: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KickReply {
    pub outcome: &'static str,
    pub reason: Option<String>,
    pub disruptions: usize,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct ActivityRequest {
    pub status: Option<String>,
}

#[derive(Serialize)]
pub struct Done {
    pub ok: bool,
}

fn point(method: &str) -> String {
    format!("/_port/{ROOM}/{method}")
}

pub fn routes<S>() -> Router<S>
where
    S: HostCtx + Clone + Send + Sync + 'static,
{
    Router::new()
        .route(&point("say"), post(say::<S>))
        .route(&point("history"), post(history::<S>))
        .route(&point("requestSwitch"), post(request_switch::<S>))
        .route(&point("startPoll"), post(start_poll::<S>))
        .route(&point("closePoll"), post(close_poll::<S>))
        .route(&point("votePoll"), post(vote_poll::<S>))
        .route(&point("kick"), post(kick::<S>))
        .route(&point("activity"), post(activity::<S>))
}

fn viewer(party: &Party, user_id: &str) -> Option<Person> {
    party
        .presence()
        .into_iter()
        .find(|v| v.user_id == user_id)
        .map(|v| Person::new(&v.user_id, &v.name))
}

async fn say<S: HostCtx + Clone>(
    State(_): State<S>,
    Extension(party): Extension<Arc<Party>>,
    Json(req): Json<SayRequest>,
) -> Json<Result<SayReply, String>> {
    port_reply(move || {
        let agent = req.speaker != "system";
        party
            .say(&req.text, agent, req.card, req.reply_to)
            .map(|id| SayReply { id })
            .map_err(|e| anyhow::anyhow!(e.code()))
    })
    .await
}

async fn history<S: HostCtx + Clone>(
    State(_): State<S>,
    Extension(party): Extension<Arc<Party>>,
    Json(req): Json<HistoryRequest>,
) -> Json<Result<HistoryReply, String>> {
    port_reply(move || {
        let limit = if req.limit == 0 { 50 } else { req.limit };
        Ok(HistoryReply { messages: party.room_history(limit, req.before) })
    })
    .await
}

async fn request_switch<S: HostCtx + Clone>(
    State(_): State<S>,
    Extension(party): Extension<Arc<Party>>,
    Json(req): Json<SwitchRequest>,
) -> Json<Result<SwitchReply, String>> {
    port_reply(move || {
        let requester = viewer(&party, &req.requested_by);
        let result = party.request_switch(Request {
            channel_id: &req.channel_id,
            channel_name: req.channel_name.as_deref(),
            requester,
            note: &req.reason,
            force: false,
        });
        Ok(SwitchReply { outcome: result.outcome, vote_id: result.vote_id, reason: result.reason })
    })
    .await
}

async fn start_poll<S: HostCtx + Clone>(
    State(_): State<S>,
    Extension(party): Extension<Arc<Party>>,
    Json(req): Json<StartPollRequest>,
) -> Json<Result<PollReply, String>> {
    port_reply(move || {
        let view = party
            .start_poll(&req.question, &req.kind, &req.options, req.duration_s)
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(PollReply { poll: serde_json::to_value(view)? })
    })
    .await
}

async fn close_poll<S: HostCtx + Clone>(
    State(_): State<S>,
    Extension(party): Extension<Arc<Party>>,
    Json(_): Json<Value>,
) -> Json<Result<ClosedPollReply, String>> {
    port_reply(move || {
        let view = party.close_poll("closedByAgent").map_err(|e| anyhow::anyhow!(e))?;
        Ok(ClosedPollReply { results: serde_json::to_value(view)? })
    })
    .await
}

async fn vote_poll<S: HostCtx + Clone>(
    State(_): State<S>,
    Extension(party): Extension<Arc<Party>>,
    Json(req): Json<VotePollRequest>,
) -> Json<Result<Done, String>> {
    port_reply(move || {
        let who = viewer(&party, &req.user_id).ok_or_else(|| anyhow::anyhow!("notPresent"))?;
        party.vote_poll(&who, &req.value).map_err(|e| anyhow::anyhow!(e))?;
        Ok(Done { ok: true })
    })
    .await
}

async fn kick<S: HostCtx + Clone>(
    State(_): State<S>,
    Extension(party): Extension<Arc<Party>>,
    Json(req): Json<KickRequest>,
) -> Json<Result<KickReply, String>> {
    port_reply(move || {
        let reason = format!("{} ({})", req.reason, req.requested_by);
        Ok(match party.kick(&req.user_id, &reason) {
            Ok(disruptions) => KickReply { outcome: "kicked", reason: None, disruptions },
            Err(code) => KickReply { outcome: "refused", reason: Some(code.into()), disruptions: 0 },
        })
    })
    .await
}

async fn activity<S: HostCtx + Clone>(
    State(_): State<S>,
    Extension(party): Extension<Arc<Party>>,
    Json(req): Json<ActivityRequest>,
) -> Json<Result<Done, String>> {
    port_reply(move || {
        party.set_activity(req.status.as_deref());
        Ok(Done { ok: true })
    })
    .await
}
