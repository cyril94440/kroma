use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::detect::FlatMatch;
use crate::live_match::MatchPayload;
use crate::peers::{ChannelRef, Viewer};
use crate::prono::{Chrono, Lag, Pronos};
use crate::sport::{ChannelLink, Memory, ScanStatus, SportEvent};

pub const SEQ_KEY: &str = "seq.reserved";
pub const MEMORY_KEY: &str = "sport.memory";
pub const PRONO_KEY: &str = "prono.state";
pub const FOLLOW_KEY: &str = "match.follow";
pub const ANNOUNCED_KEY: &str = "announced";
pub const LINKS_KEY: &str = "sport.links";

#[derive(Debug, Default)]
pub struct Sweep {
    pub queue: Vec<(String, String)>,
    pub responses: BTreeMap<String, Option<Value>>,
    pub far_included: bool,
}

#[derive(Debug, Default)]
pub struct Events {
    pub list: Vec<SportEvent>,
    pub read_at: f64,
    pub far_at: f64,
    pub far: BTreeMap<String, Value>,
    pub sweep: Option<Sweep>,
    pub force: bool,
}

#[derive(Debug, Default)]
pub struct Guide {
    pub links: BTreeMap<String, ChannelLink>,
    pub empty: BTreeMap<String, f64>,
    pub preference: BTreeMap<String, f64>,
    pub preference_at: f64,
    pub relink_all: bool,
}

#[derive(Debug, Default)]
pub struct Board {
    pub days: BTreeMap<String, (f64, Vec<FlatMatch>)>,
}

#[derive(Debug, Clone)]
pub struct Matched {
    pub found: FlatMatch,
    pub channel_id: String,
}

#[derive(Debug, Default)]
pub struct Detect {
    pub last_run: f64,
    pub force: bool,
    pub matched: Option<Matched>,
    pub post_seen: BTreeMap<String, f64>,
    pub last_verdict: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FollowSaved {
    pub match_id: String,
    pub seen: BTreeMap<String, i64>,
    pub chronos: Vec<Chrono>,
}

#[derive(Debug, Default)]
pub struct Follow {
    pub target: Option<String>,
    pub payload: Option<MatchPayload>,
    pub summary: Value,
    pub saved: FollowSaved,
    pub last_fetch: f64,
    pub post_since: Option<f64>,
    pub published: bool,
    pub minute: Option<(i64, f64)>,
    pub change: Option<(f64, f64)>,
    pub espn_state: String,
    pub espn_period: i64,
    pub espn_clock_s: Option<i64>,
    pub half_time: bool,
}

#[derive(Debug, Default)]
pub struct ClipsState {
    pub done: BTreeMap<String, f64>,
    pub attempts: BTreeMap<String, u32>,
    pub buffer_from_s: Option<f64>,
    pub buffer_at: f64,
    pub unsupported_until: f64,
    pub last_cut: f64,
    pub states: BTreeMap<String, String>,
}

#[derive(Debug, Default)]
pub struct ScanState {
    pub status: ScanStatus,
    pub last_scan: f64,
    pub poll_at: f64,
    pub opened_at: f64,
    pub forced_at: f64,
}

#[derive(Debug, Default)]
pub struct Presence {
    pub viewers: Vec<Viewer>,
    pub channel: Option<ChannelRef>,
    pub provider: String,
    pub started_at: Option<i64>,
    pub at: f64,
    pub known: bool,
}

#[derive(Debug, Default)]
pub struct Standings {
    pub cache: BTreeMap<String, (f64, Value)>,
    pub pending: BTreeSet<String>,
}

pub struct State {
    pub seq: u64,
    pub reserved: u64,
    pub presence: Presence,
    pub events: Events,
    pub guide: Guide,
    pub memory: Memory,
    pub scan: ScanState,
    pub board: Board,
    pub detect: Detect,
    pub follow: Follow,
    pub pronos: Pronos,
    pub lag: Lag,
    pub prono_at: f64,
    pub standings: Standings,
    pub clips: ClipsState,
    pub announced: BTreeSet<String>,
    pub views: BTreeMap<&'static str, String>,
    pub admins: BTreeMap<String, i64>,
}
