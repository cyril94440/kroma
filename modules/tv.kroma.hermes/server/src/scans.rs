use std::collections::VecDeque;
use std::sync::Mutex;

use crate::matching::{has_teams, teams_of, Verdict};
use crate::model::ScreenReading;

mod groups;
#[cfg(test)]
mod tests;

pub use groups::{build_groups, Group};

const TTL_MS: i64 = 1_800_000;
const KEEP: usize = 5;
pub const LIVE_MEMORY_MS: i64 = 600_000;
pub const VERDICT_MAX_AGE_MS: i64 = 900_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowState {
    Ok,
    Black,
    Error,
    NotTested,
}

impl RowState {
    pub fn label(self) -> &'static str {
        match self {
            RowState::Ok => "ok",
            RowState::Black => "noir",
            RowState::Error => "erreur",
            RowState::NotTested => "non_teste",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Row {
    pub index: usize,
    pub id: String,
    pub name: String,
    pub number: Option<u32>,
    pub state: RowState,
    pub reason: String,
    pub reading: Option<ScreenReading>,
    pub verdict: Option<Verdict>,
    pub why: String,
    pub target: Option<Verdict>,
    pub target_why: String,
}

#[derive(Debug, Clone)]
pub struct Scan {
    pub id: u32,
    pub at: i64,
    pub provider: String,
    pub query: String,
    pub compared: bool,
    pub search: String,
    pub live: Option<String>,
    pub by: String,
    pub ref_teams: Vec<String>,
    pub ref_at: Option<i64>,
    pub rows: Vec<Row>,
    pub groups: Vec<Group>,
}

#[derive(Debug, Clone)]
pub struct Seen {
    pub channel: String,
    pub reading: ScreenReading,
    pub at: i64,
}

#[derive(Default)]
pub struct LiveSeen {
    last_teams: Mutex<Option<Seen>>,
}

impl LiveSeen {
    pub fn set(&self, channel: &str, reading: &ScreenReading, at: i64) {
        if has_teams(reading) {
            *self.last_teams.lock().unwrap_or_else(|e| e.into_inner()) = Some(Seen { channel: channel.into(), reading: reading.clone(), at });
        }
    }

    pub fn teams(&self, channel: &str, now: i64, max_age_ms: i64) -> Option<Seen> {
        self.last_teams.lock().unwrap_or_else(|e| e.into_inner()).clone().filter(|s| s.channel == channel && now - s.at <= max_age_ms)
    }
}

#[derive(Default)]
pub struct ScanCache {
    scans: Mutex<VecDeque<Scan>>,
    seq: Mutex<u32>,
}

pub struct Judged {
    pub verdict: Verdict,
    pub scan_at: i64,
    pub ref_teams: Vec<String>,
    pub ref_at: Option<i64>,
}

pub fn team_names(reading: &ScreenReading) -> Vec<String> {
    teams_of(reading).into_iter().flatten().collect()
}

impl ScanCache {
    pub fn next_id(&self) -> u32 {
        let mut seq = self.seq.lock().unwrap_or_else(|e| e.into_inner());
        *seq += 1;
        *seq
    }

    pub fn add(&self, scan: Scan) {
        let mut scans = self.scans.lock().unwrap_or_else(|e| e.into_inner());
        scans.push_back(scan);
        while scans.len() > KEEP {
            scans.pop_front();
        }
    }

    fn alive(&self, now: i64) -> Vec<Scan> {
        self.scans.lock().unwrap_or_else(|e| e.into_inner()).iter().filter(|s| now - s.at <= TTL_MS).cloned().collect()
    }

    pub fn resolve_ref(&self, reference: &str, now: i64) -> Option<String> {
        let body = reference.trim().strip_prefix('#')?;
        let (scan_id, group): (Option<u32>, usize) = match body.split_once('.') {
            Some((s, g)) => (Some(s.parse().ok()?), g.parse().ok()?),
            None => (None, body.parse().ok()?),
        };
        let alive = self.alive(now);
        let scan = match scan_id {
            Some(id) => alive.into_iter().find(|s| s.id == id)?,
            None if alive.len() == 1 => alive.into_iter().next()?,
            None => return None,
        };
        scan.groups.iter().find(|g| g.n == group).and_then(|g| g.members.first()).map(|m| m.id.clone())
    }

    pub fn judged(&self, channel_id: &str, live: &str, now: i64) -> Option<Judged> {
        self.alive(now).into_iter().rev().filter(|s| s.compared && s.live.as_deref() == Some(live) && now - s.at <= VERDICT_MAX_AGE_MS).find_map(|s| {
            let verdict = s.rows.iter().find(|r| r.id == channel_id).and_then(|r| r.verdict)?;
            Some(Judged { verdict, scan_at: s.at, ref_teams: s.ref_teams, ref_at: s.ref_at })
        })
    }

    pub fn seen_alive(&self, channel_id: &str, now: i64, max_age_ms: i64) -> bool {
        self.alive(now).iter().any(|s| now - s.at <= max_age_ms && s.rows.iter().any(|r| r.id == channel_id && r.state == RowState::Ok))
    }

    pub fn summary_lines(&self, now: i64) -> Vec<String> {
        self.alive(now).last().map(|s| vec![summary(s)]).unwrap_or_default()
    }
}

fn summary(scan: &Scan) -> String {
    let bits: Vec<String> = scan
        .groups
        .iter()
        .take(4)
        .map(|g| match g.members.first() {
            Some(m) => format!("#{}.{} {} ({}) → {} {}", scan.id, g.n, g.content, g.kind, m.id, m.name),
            None => format!("#{}.{} {} ({})", scan.id, g.n, g.content, g.kind),
        })
        .collect();
    format!("[état] Scan #{} (« {} ») : {}", scan.id, scan.query, if bits.is_empty() { "aucune image".to_string() } else { bits.join(" ; ") })
}
