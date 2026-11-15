use std::sync::Arc;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

use kroma_module_sdk::host::{call_point, pinned_resolver, HostCtx};

mod proposal;

pub(crate) use proposal::ProposalEvent;

pub const CONTROL: &str = "tv.kroma.live/control";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ChannelRef {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Programme {
    pub title: String,
    pub start: i64,
    pub end: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Health {
    pub source_kbps: f64,
    pub stutter: bool,
    pub black: bool,
    pub frozen: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Viewer {
    pub user_id: String,
    pub name: String,
    pub since: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Status {
    pub state: String,
    pub channel: Option<ChannelRef>,
    pub provider: String,
    pub now: Option<Programme>,
    pub next: Option<Programme>,
    pub viewers: Vec<Viewer>,
    pub health: Health,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Switched {
    pub switched: bool,
    pub channel: Option<ChannelRef>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Candidate {
    pub id: String,
    pub name: String,
    pub group: String,
    pub score: f64,
    pub now: Option<Programme>,
}

pub trait Live: Send + Sync {
    fn status(&self) -> Result<Status>;
    fn presence(&self) -> Result<Vec<Viewer>>;
    fn switch(&self, channel_id: &str, requested_by: &str, reason: &str) -> Result<Switched>;
    fn proposal(&self, id: &str, accept: bool) -> Result<bool>;
    fn resolve(&self, query: &str, limit: u32) -> Result<Vec<Candidate>>;
}

pub struct PeerLive {
    host: Arc<dyn HostCtx>,
}

impl PeerLive {
    pub fn new(host: Arc<dyn HostCtx>) -> Self {
        Self { host }
    }

    fn ask<B: Serialize, T: serde::de::DeserializeOwned>(&self, method: &str, body: &B) -> Result<T> {
        let resolve = pinned_resolver(&*self.host, CONTROL, None)
            .ok_or_else(|| anyhow!("no module answers {CONTROL}"))?;
        call_point(&resolve, CONTROL, method, body)
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Viewers {
    viewers: Vec<Viewer>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Applied {
    applied: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Candidates {
    channels: Vec<Candidate>,
}

impl Live for PeerLive {
    fn status(&self) -> Result<Status> {
        self.ask("status", &json!({}))
    }

    fn presence(&self) -> Result<Vec<Viewer>> {
        Ok(self.ask::<_, Viewers>("presence", &json!({}))?.viewers)
    }

    fn switch(&self, channel_id: &str, requested_by: &str, reason: &str) -> Result<Switched> {
        self.ask(
            "switch",
            &json!({ "channelId": channel_id, "requestedBy": requested_by, "reason": reason }),
        )
    }

    fn proposal(&self, id: &str, accept: bool) -> Result<bool> {
        Ok(self.ask::<_, Applied>("proposal", &json!({ "id": id, "accept": accept }))?.applied)
    }

    fn resolve(&self, query: &str, limit: u32) -> Result<Vec<Candidate>> {
        Ok(self
            .ask::<_, Candidates>("resolve", &json!({ "query": query, "limit": limit }))?
            .channels)
    }
}

#[cfg(test)]
mod tests;
