use std::sync::Arc;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use kroma_module_sdk::host::{call_point, pinned_resolver, HostCtx};

pub const ROOM: &str = "tv.kroma.party/room";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RoomMessage {
    pub id: String,
    pub user_id: Option<String>,
    pub name: String,
    pub kind: String,
    pub text: String,
    pub at: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SwitchOutcome {
    pub outcome: String,
    pub vote_id: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct KickOutcome {
    pub outcome: String,
    pub reason: Option<String>,
    pub disruptions: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct SwitchAsk<'a> {
    pub channel_id: &'a str,
    pub channel_name: Option<&'a str>,
    pub requested_by: &'a str,
    pub reason: &'a str,
    pub force: bool,
}

/// The shared room as this module uses it, over the `tv.kroma.party/room` point.
pub trait Room: Send + Sync {
    fn say(&self, text: &str, card: Option<Value>, reply_to: Option<&str>) -> Result<String>;
    fn history(&self, limit: u32, before: Option<i64>) -> Result<Vec<RoomMessage>>;
    fn request_switch(&self, ask: &SwitchAsk) -> Result<SwitchOutcome>;
    fn start_poll(&self, question: &str, kind: &str, options: &[String], duration_s: Option<i64>) -> Result<Value>;
    fn close_poll(&self) -> Result<Value>;
    fn vote_poll(&self, user_id: &str, value: &str) -> Result<()>;
    fn kick(&self, user_id: &str, requested_by: &str, reason: &str) -> Result<KickOutcome>;
    fn activity(&self, status: Option<&str>) -> Result<()>;
    fn go_live(&self, requested_by: &str) -> Result<Value>;
    fn fine_sync(&self, requested_by: &str) -> Result<Value>;
    fn client_logs(&self, user_id: Option<&str>, limit: u32, minutes: Option<u32>) -> Result<Vec<Value>>;
}

pub struct PeerRoom {
    host: Arc<dyn HostCtx>,
}

impl PeerRoom {
    pub fn new(host: Arc<dyn HostCtx>) -> Self {
        Self { host }
    }

    fn ask<T: serde::de::DeserializeOwned>(&self, method: &str, body: &Value) -> Result<T> {
        let resolve = pinned_resolver(&*self.host, ROOM, None).ok_or_else(|| anyhow!("no module contributes {ROOM}"))?;
        call_point(&resolve, ROOM, method, body)
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Said {
    id: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Messages {
    messages: Vec<RoomMessage>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Poll {
    poll: Value,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Closed {
    results: Value,
}

#[derive(Deserialize, Default)]
struct Ignored {}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Logs {
    events: Vec<Value>,
}

impl Room for PeerRoom {
    fn say(&self, text: &str, card: Option<Value>, reply_to: Option<&str>) -> Result<String> {
        let said: Said = self.ask("say", &json!({ "text": text, "as": "agent", "card": card, "replyTo": reply_to }))?;
        Ok(said.id)
    }

    fn history(&self, limit: u32, before: Option<i64>) -> Result<Vec<RoomMessage>> {
        Ok(self.ask::<Messages>("history", &json!({ "limit": limit, "before": before }))?.messages)
    }

    fn request_switch(&self, ask: &SwitchAsk) -> Result<SwitchOutcome> {
        let mut body = json!({ "channelId": ask.channel_id, "requestedBy": ask.requested_by, "reason": ask.reason });
        if let Some(name) = ask.channel_name {
            body["channelName"] = json!(name);
        }
        if ask.force {
            body["force"] = json!(true);
        }
        self.ask("requestSwitch", &body)
    }

    fn start_poll(&self, question: &str, kind: &str, options: &[String], duration_s: Option<i64>) -> Result<Value> {
        Ok(self.ask::<Poll>("startPoll", &json!({ "question": question, "kind": kind, "options": options, "durationS": duration_s }))?.poll)
    }

    fn close_poll(&self) -> Result<Value> {
        Ok(self.ask::<Closed>("closePoll", &json!({}))?.results)
    }

    fn vote_poll(&self, user_id: &str, value: &str) -> Result<()> {
        self.ask::<Ignored>("votePoll", &json!({ "userId": user_id, "value": value })).map(|_| ())
    }

    fn kick(&self, user_id: &str, requested_by: &str, reason: &str) -> Result<KickOutcome> {
        self.ask("kick", &json!({ "userId": user_id, "requestedBy": requested_by, "reason": reason }))
    }

    fn activity(&self, status: Option<&str>) -> Result<()> {
        self.ask::<Ignored>("activity", &json!({ "status": status })).map(|_| ())
    }

    fn go_live(&self, requested_by: &str) -> Result<Value> {
        self.ask("goLive", &json!({ "requestedBy": requested_by }))
    }

    fn fine_sync(&self, requested_by: &str) -> Result<Value> {
        self.ask("fineSync", &json!({ "requestedBy": requested_by }))
    }

    fn client_logs(&self, user_id: Option<&str>, limit: u32, minutes: Option<u32>) -> Result<Vec<Value>> {
        let mut body = json!({ "userId": user_id, "limit": limit });
        if let Some(minutes) = minutes {
            body["minutes"] = json!(minutes);
        }
        Ok(self.ask::<Logs>("clientLogs", &body)?.events)
    }
}
