use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::RoomMessage;

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
    pub force: bool,
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
    pub requested_by: String,
    pub question: String,
    pub kind: String,
    pub options: Vec<String>,
    pub duration_s: Option<i64>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct ClosePollRequest {
    pub requested_by: String,
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
#[serde(default, rename_all = "camelCase")]
pub struct SyncActionRequest {
    pub requested_by: String,
}

#[derive(Serialize)]
pub struct GoLiveReply {
    pub members: usize,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct ClientLogsRequest {
    pub requested_by: String,
    pub user_id: Option<String>,
    pub limit: Option<u32>,
    pub minutes: Option<i64>,
}

#[derive(Serialize)]
pub struct ClientLogsReply {
    pub events: Vec<crate::store::LogEntry>,
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
