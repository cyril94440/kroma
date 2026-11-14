use std::sync::Arc;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use kroma_module_sdk::host::{call_point, pinned_resolver, HostCtx};

pub const CONTROL: &str = "tv.kroma.live/control";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Slot {
    pub title: String,
    pub start: i64,
    pub end: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ChannelRef {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Health {
    pub source_kbps: f64,
    pub stutter: bool,
    pub black: bool,
    pub frozen: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Viewer {
    pub user_id: String,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Status {
    pub state: String,
    pub channel: Option<ChannelRef>,
    pub provider: String,
    pub now: Option<Slot>,
    pub next: Option<Slot>,
    pub viewers: Vec<Viewer>,
    pub health: Health,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Candidate {
    pub id: String,
    pub name: String,
    pub group: String,
    pub score: f64,
    pub now: Option<Slot>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Programme {
    pub channel_id: String,
    pub channel_name: String,
    pub title: String,
    pub description: String,
    pub start: i64,
    pub end: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Snapshot {
    pub jpeg_base64: String,
    pub at: i64,
}

/// The live television as this module uses it, over the `tv.kroma.live/control`
/// point. It never switches channel itself: a change goes through the room's vote.
pub trait Control: Send + Sync {
    fn status(&self) -> Result<Status>;
    fn resolve(&self, query: &str, limit: u32) -> Result<Vec<Candidate>>;
    fn epg(&self, query: Option<&str>, from: Option<i64>, to: Option<i64>, limit: u32) -> Result<Vec<Programme>>;
    fn snapshot(&self) -> Result<Snapshot>;
}

pub struct PeerControl {
    host: Arc<dyn HostCtx>,
}

impl PeerControl {
    pub fn new(host: Arc<dyn HostCtx>) -> Self {
        Self { host }
    }

    fn ask<T: serde::de::DeserializeOwned>(&self, method: &str, body: &Value) -> Result<T> {
        let resolve = pinned_resolver(&*self.host, CONTROL, None).ok_or_else(|| anyhow!("no module contributes {CONTROL}"))?;
        call_point(&resolve, CONTROL, method, body)
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Channels {
    channels: Vec<Candidate>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Programmes {
    programmes: Vec<Programme>,
}

impl Control for PeerControl {
    fn status(&self) -> Result<Status> {
        self.ask("status", &json!({}))
    }

    fn resolve(&self, query: &str, limit: u32) -> Result<Vec<Candidate>> {
        Ok(self.ask::<Channels>("resolve", &json!({ "query": query, "limit": limit }))?.channels)
    }

    fn epg(&self, query: Option<&str>, from: Option<i64>, to: Option<i64>, limit: u32) -> Result<Vec<Programme>> {
        Ok(self.ask::<Programmes>("epg", &json!({ "query": query, "from": from, "to": to, "limit": limit }))?.programmes)
    }

    fn snapshot(&self) -> Result<Snapshot> {
        self.ask("snapshot", &json!({}))
    }
}
