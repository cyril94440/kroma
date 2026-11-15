use std::sync::Arc;

use axum::extract::{Extension, State};
use axum::routing::post;
use axum::{Json, Router};

use kroma_module_sdk::host::{port_reply, HostCtx};

use crate::model::Person;
use crate::party::{Actor, Party, Request};

mod wire;

pub use wire::*;

pub const ROOM: &str = "tv.kroma.party/room";

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
        .route(&point("goLive"), post(go_live::<S>))
        .route(&point("fineSync"), post(fine_sync::<S>))
        .route(&point("clientLogs"), post(client_logs::<S>))
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
        let force = req.force
            && party.may_bypass(&Actor { person: Person::new(&req.requested_by, ""), admin: party.is_admin_id(&req.requested_by) });
        let result = party.request_switch(Request {
            channel_id: &req.channel_id,
            channel_name: req.channel_name.as_deref(),
            requester,
            note: &req.reason,
            force,
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
        party.require_admin_id(&req.requested_by).map_err(|e| anyhow::anyhow!(e))?;
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
    Json(req): Json<ClosePollRequest>,
) -> Json<Result<ClosedPollReply, String>> {
    port_reply(move || {
        party.require_admin_id(&req.requested_by).map_err(|e| anyhow::anyhow!(e))?;
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
        if let Err(code) = party.require_admin_id(&req.requested_by) {
            return Ok(KickReply { outcome: "refused", reason: Some(code.into()), disruptions: 0 });
        }
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

async fn go_live<S: HostCtx + Clone>(
    State(_): State<S>,
    Extension(party): Extension<Arc<Party>>,
    Json(req): Json<SyncActionRequest>,
) -> Json<Result<GoLiveReply, String>> {
    port_reply(move || {
        party.require_admin_id(&req.requested_by).map_err(|e| anyhow::anyhow!(e))?;
        party.sync_go_live(&req.requested_by).map(|members| GoLiveReply { members }).map_err(|e| anyhow::anyhow!(e))
    })
    .await
}

async fn fine_sync<S: HostCtx + Clone>(
    State(_): State<S>,
    Extension(party): Extension<Arc<Party>>,
    Json(req): Json<SyncActionRequest>,
) -> Json<Result<Done, String>> {
    port_reply(move || {
        party.require_admin_id(&req.requested_by).map_err(|e| anyhow::anyhow!(e))?;
        party.sync_fine_sync(&req.requested_by).map(|()| Done { ok: true }).map_err(|e| anyhow::anyhow!(e))
    })
    .await
}

async fn client_logs<S: HostCtx + Clone>(
    State(_): State<S>,
    Extension(party): Extension<Arc<Party>>,
    Json(req): Json<ClientLogsRequest>,
) -> Json<Result<ClientLogsReply, String>> {
    port_reply(move || {
        party.require_log_access(&req.requested_by, req.user_id.as_deref()).map_err(|e| anyhow::anyhow!(e))?;
        let events = party.read_logs(req.minutes.unwrap_or(30), req.user_id.as_deref(), None, req.limit.unwrap_or(120));
        Ok(ClientLogsReply { events })
    })
    .await
}
