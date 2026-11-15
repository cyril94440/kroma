use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Restart {
    pub restarted: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ProviderSwitch {
    pub switched: bool,
    pub provider: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Probe {
    pub channel_id: String,
    pub name: String,
    pub provider: Option<String>,
    pub alive: bool,
    pub resolution: Option<String>,
    pub codec: Option<String>,
    pub audio_codec: Option<String>,
    pub fps: Option<f64>,
    pub bitrate_kbps: Option<f64>,
    pub off_air: bool,
    pub local: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BenchmarkStart {
    pub status: String,
    pub id: Option<String>,
    pub reason: Option<String>,
    pub total: usize,
    pub already_done: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ScanItem {
    pub number: Option<u32>,
    pub id: String,
    pub name: String,
    pub ok: bool,
    pub black: bool,
    pub still: bool,
    pub frame: Option<String>,
    pub frames: Vec<String>,
    pub error: Option<String>,
    pub now: Option<String>,
    pub at: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ScanState {
    pub id: String,
    pub query: String,
    pub running: bool,
    pub provider: Option<String>,
    pub items: Vec<ScanItem>,
    pub total: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HistoryPoint {
    pub at: i64,
    pub kbps: f64,
    pub channel: Option<String>,
    pub provider: Option<String>,
    pub health: Option<String>,
    pub loss: f64,
    pub frozen: bool,
    pub black: bool,
    pub restarts: i64,
    pub paused: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct History {
    pub step_seconds: i64,
    pub points: Vec<HistoryPoint>,
    pub provider_events: Vec<Value>,
}
