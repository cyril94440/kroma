use serde::{Deserialize, Serialize};

use crate::model::{Programme, ProviderId};
use crate::presence::Viewer;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    #[default]
    Paused,
    Running,
    Switching,
    OffAir,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ChannelRef {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Slot {
    pub title: String,
    pub start: i64,
    pub end: i64,
}

impl Slot {
    pub fn of(programme: &Programme) -> Slot {
        Slot { title: programme.title.clone(), start: programme.start, end: programme.end }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct HealthView {
    pub source_kbps: f64,
    pub stutter: bool,
    pub black: bool,
    pub frozen: bool,
    pub trouble: bool,
    pub state: String,
    pub loss_seconds: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct OffAirView {
    pub name: String,
    pub duration: Option<f64>,
    pub at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub state: Phase,
    pub channel: Option<ChannelRef>,
    pub provider: ProviderId,
    pub on_backup: bool,
    pub now: Option<Slot>,
    pub next: Option<Slot>,
    pub viewers: Vec<Viewer>,
    pub health: HealthView,
    pub playlist: Option<String>,
    pub started_at: Option<i64>,
    pub restarts: u64,
    pub standby_in_seconds: Option<u64>,
    pub last_error: Option<String>,
    pub off_air: Option<OffAirView>,
    pub backup_configured: bool,
    pub primary_down: bool,
    pub source_resolution: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Proposal {
    pub id: String,
    pub kind: String,
    pub text: String,
    pub channel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_by: Option<String>,
    pub created_at: i64,
    pub expires_at: i64,
}
