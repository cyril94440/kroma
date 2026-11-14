use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::model::{params, Person};

mod ops;
mod tick;

pub const CAP_S: f64 = 12.0;
pub const KICK_WINDOW_MS: i64 = 30 * 60 * 1000;
pub const KICK_COOLDOWN_MS: i64 = 2 * 60 * 1000;
pub const GOLIVE_STALE_MS: i64 = 4000;
pub const ABSENT_MS: i64 = 8000;
pub const PURGE_MS: i64 = 45_000;
pub const BAND_S: f64 = 0.5;
pub const STUCK_WINDOW_MS: i64 = 20_000;
pub const STUCK_THRESHOLD: usize = 5;
pub const ANNOUNCE_GAP_MS: i64 = 120_000;
pub const RECAL_GAP_MS: i64 = 5000;
pub const FINE_FREEZE_MS: i64 = 60_000;
pub const FINE_REPORT_TIMEOUT_MS: i64 = 75_000;
const JOURNAL_MAX: usize = 100;
pub(crate) const RECAL_MAX: usize = 60;

#[derive(Debug, Clone, PartialEq)]
pub struct Say {
    pub key: &'static str,
    pub params: Map<String, Value>,
    pub action: Option<&'static str>,
}

impl Say {
    fn new(key: &'static str, pairs: &[(&str, &str)], action: Option<&'static str>) -> Self {
        Self {
            key,
            params: params(pairs),
            action,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Beat {
    pub enabled: bool,
    pub delay: f64,
    pub paused: bool,
    pub pinned: bool,
    pub blocked: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FineDone {
    pub ms: i64,
    pub seeks: i64,
    pub abandoned: bool,
    pub solo: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Recal {
    pub gap: f64,
    pub target: f64,
    pub arrival: bool,
    pub blocked: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct Member {
    pub name: String,
    pub delay: f64,
    pub paused: bool,
    pub pinned: bool,
    pub blocked: bool,
    pub paused_at: i64,
    pub ts: i64,
}

#[derive(Debug, Clone)]
pub(crate) struct FineReports {
    pub(crate) expected: BTreeSet<String>,
    pub(crate) got: BTreeMap<String, Option<(i64, i64)>>,
    pub(crate) names: BTreeMap<String, String>,
    pub(crate) deadline: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncState {
    pub target: f64,
    pub by: Option<Person>,
    pub capped: bool,
    pub members: Vec<Person>,
    pub active: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum KickOutcome {
    Kicked { disruptions: usize, reason_stated: bool },
    Refused(&'static str),
}

#[derive(Debug)]
pub struct SyncGroup {
    pub(crate) members: BTreeMap<String, Member>,
    pub(crate) announced: BTreeMap<String, i64>,
    pub(crate) journal: VecDeque<Value>,
    pub(crate) stuck: BTreeMap<String, Vec<i64>>,
    pub(crate) recalibrations: VecDeque<Value>,
    pub(crate) last_kick: BTreeMap<String, i64>,
    pub(crate) was_active: bool,
    pub(crate) target: f64,
    pub(crate) by: Option<String>,
    pub(crate) golive_at: i64,
    pub(crate) freeze_until: i64,
    pub(crate) reports: Option<FineReports>,
    pub(crate) quiet_until: i64,
}

impl SyncGroup {
    pub fn new(quiet_until: i64) -> Self {
        Self {
            members: BTreeMap::new(),
            announced: BTreeMap::new(),
            journal: VecDeque::new(),
            stuck: BTreeMap::new(),
            recalibrations: VecDeque::new(),
            last_kick: BTreeMap::new(),
            was_active: false,
            target: 0.0,
            by: None,
            golive_at: 0,
            freeze_until: 0,
            reports: None,
            quiet_until,
        }
    }

    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    #[cfg(test)]
    pub fn contains(&self, user_id: &str) -> bool {
        self.members.contains_key(user_id)
    }

    #[cfg(test)]
    pub fn target(&self) -> f64 {
        self.target
    }

    pub fn journal(&self) -> Vec<Value> {
        self.journal.iter().cloned().collect()
    }

    pub fn recalibrations(&self) -> Vec<Value> {
        self.recalibrations.iter().cloned().collect()
    }

    pub(crate) fn log(&mut self, now: i64, mut event: Value) {
        if let Some(map) = event.as_object_mut() {
            map.insert("ts".into(), json!(now));
        }
        self.journal.push_back(event);
        if self.journal.len() > JOURNAL_MAX {
            self.journal.pop_front();
        }
    }

    pub(crate) fn fresh(&self, m: &Member, now: i64) -> bool {
        now - m.ts <= ABSENT_MS && m.ts >= self.golive_at + GOLIVE_STALE_MS
    }

    pub fn remove(&mut self, user_id: &str, now: i64) {
        if let Some(m) = self.members.remove(user_id) {
            self.log(now, json!({ "event": "quitte", "userId": user_id, "name": m.name }));
            self.stuck.remove(user_id);
        }
    }

    pub fn beat(&mut self, who: &Person, beat: &Beat, now: i64) -> Vec<Say> {
        if !beat.enabled {
            self.remove(&who.user_id, now);
            return Vec::new();
        }
        let was_empty = self.members.is_empty();
        let previous = self.members.get(&who.user_id).map(|m| m.paused_at);
        let is_new = previous.is_none();
        self.members.insert(
            who.user_id.clone(),
            Member {
                name: who.name.clone(),
                delay: beat.delay.max(0.0),
                paused: beat.paused,
                pinned: beat.pinned,
                blocked: beat.blocked,
                paused_at: if beat.paused { now } else { previous.unwrap_or(0) },
                ts: now,
            },
        );
        let mut says = Vec::new();
        if is_new {
            self.log(now, json!({ "event": "rejoint", "userId": who.user_id, "name": who.name }));
            let last = self.announced.get(&who.user_id).copied().unwrap_or(0);
            if now >= self.quiet_until && now - last > ANNOUNCE_GAP_MS {
                self.announced.insert(who.user_id.clone(), now);
                let key = if was_empty { "party.sync.started" } else { "party.sync.joined" };
                says.push(Say::new(key, &[("name", &who.name)], Some("join_party")));
            }
        }
        says
    }
}

#[cfg(test)]
mod tests;
