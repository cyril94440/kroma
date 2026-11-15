use std::sync::Arc;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

use kroma_module_sdk::host::{call_point, pinned_resolver, HostCtx};

use super::is_unsupported;
use crate::sport::ScanRow;

pub const CONTROL: &str = "tv.kroma.live/control";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ChannelRef {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Slot {
    pub title: String,
    pub start: i64,
    pub end: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Viewer {
    pub user_id: String,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LiveStatus {
    pub state: String,
    pub channel: Option<ChannelRef>,
    pub provider: String,
    pub now: Option<Slot>,
    pub next: Option<Slot>,
    pub viewers: Vec<Viewer>,
    pub started_at: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Programme {
    pub channel_id: String,
    pub channel_name: String,
    pub title: String,
    pub description: String,
    pub start: i64,
    pub end: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Candidate {
    pub id: String,
    pub name: String,
    pub group: String,
    pub score: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BufferInfo {
    pub from_ms: Option<i64>,
    pub to_ms: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CutClip {
    pub path: String,
    pub duration_ms: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct ScanItem {
    number: Option<i64>,
    id: String,
    name: String,
    ok: bool,
    black: bool,
    now: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ScanState {
    running: bool,
    total: usize,
    items: Vec<ScanItem>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScanReport {
    pub running: bool,
    pub total: usize,
    pub rows: Vec<ScanRow>,
}

impl From<ScanState> for ScanReport {
    fn from(state: ScanState) -> Self {
        let rows = state
            .items
            .into_iter()
            .map(|item| ScanRow {
                state: if item.black {
                    "noir"
                } else if item.ok {
                    "ok"
                } else {
                    "erreur"
                }
                .into(),
                reading: item
                    .now
                    .as_deref()
                    .filter(|_| item.ok && !item.black)
                    .and_then(crate::sport::reading_from_title),
                channel_id: item.id,
                name: Some(item.name),
                num: item.number,
                from_guide: true,
            })
            .collect();
        Self {
            running: state.running,
            total: state.total,
            rows,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Benchmarked {
    pub status: String,
    pub reason: Option<String>,
    pub already_done: bool,
}

pub trait Live: Send + Sync {
    fn status(&self) -> Result<LiveStatus>;
    fn epg(&self, query: &str, from_ms: i64, to_ms: i64, limit: u32) -> Result<Vec<Programme>>;
    fn resolve(&self, query: &str, limit: u32) -> Result<Vec<Candidate>>;
    fn clip_buffer_info(&self) -> Result<Option<BufferInfo>>;
    fn clip_cut(&self, from_ms: i64, to_ms: i64, name: &str) -> Result<Option<CutClip>>;
    fn clip_delete(&self, path: &str) -> Result<bool>;
    fn clip_list(&self) -> Result<Option<Vec<String>>>;
    fn scan_result(&self) -> Result<Option<ScanReport>>;
    fn benchmark(&self, channel_ids: &[String], requested_by: &str) -> Result<Option<Benchmarked>>;
}

pub struct PeerLive {
    host: Arc<dyn HostCtx>,
}

impl PeerLive {
    pub fn new(host: Arc<dyn HostCtx>) -> Self {
        Self { host }
    }

    fn ask<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        body: &serde_json::Value,
    ) -> Result<T> {
        let resolve = pinned_resolver(&*self.host, CONTROL, None)
            .ok_or_else(|| anyhow!("no module answers {CONTROL}"))?;
        call_point(&resolve, CONTROL, method, body)
    }

    fn optional<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        body: &serde_json::Value,
    ) -> Result<Option<T>> {
        match self.ask(method, body) {
            Ok(value) => Ok(Some(value)),
            Err(e) if is_unsupported(&e) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Programmes {
    programmes: Vec<Programme>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Candidates {
    channels: Vec<Candidate>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Paths {
    paths: Vec<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Deleted {
    deleted: bool,
}

impl Live for PeerLive {
    fn status(&self) -> Result<LiveStatus> {
        self.ask("status", &json!({}))
    }

    fn epg(&self, query: &str, from_ms: i64, to_ms: i64, limit: u32) -> Result<Vec<Programme>> {
        Ok(self
            .ask::<Programmes>(
                "epg",
                &json!({"query": query, "from": from_ms, "to": to_ms, "limit": limit}),
            )?
            .programmes)
    }

    fn resolve(&self, query: &str, limit: u32) -> Result<Vec<Candidate>> {
        Ok(self
            .ask::<Candidates>("resolve", &json!({"query": query, "limit": limit}))?
            .channels)
    }

    fn clip_buffer_info(&self) -> Result<Option<BufferInfo>> {
        self.optional("clipBufferInfo", &json!({}))
    }

    fn clip_cut(&self, from_ms: i64, to_ms: i64, name: &str) -> Result<Option<CutClip>> {
        self.optional(
            "clipCut",
            &json!({"fromMs": from_ms, "toMs": to_ms, "name": name}),
        )
    }

    fn clip_delete(&self, path: &str) -> Result<bool> {
        Ok(self
            .optional::<Deleted>("clipDelete", &json!({"path": path}))?
            .is_some_and(|d| d.deleted))
    }

    fn clip_list(&self) -> Result<Option<Vec<String>>> {
        Ok(self
            .optional::<Paths>("clipList", &json!({}))?
            .map(|p| p.paths))
    }

    fn scan_result(&self) -> Result<Option<ScanReport>> {
        Ok(self
            .optional::<ScanState>("scanResult", &json!({}))?
            .map(ScanReport::from))
    }

    fn benchmark(&self, channel_ids: &[String], requested_by: &str) -> Result<Option<Benchmarked>> {
        self.optional(
            "benchmark",
            &json!({"channelIds": channel_ids, "requestedBy": requested_by}),
        )
    }
}
