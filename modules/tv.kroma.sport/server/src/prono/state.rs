use std::collections::BTreeMap;

use super::calibrage::{Calibrage, DirectKey};
use super::model::{Current, File, Season, Status};
use super::score::parse_score;
use crate::detect::FlatMatch;

pub const PRONO_TICK_S: f64 = 60.0;
pub const BEFORE_KICKOFF_S: f64 = 20.0 * 60.0;
pub const MAX_MINUTE_IN_MATCH: i64 = 10;
pub const MISSED_MAX: u32 = 15;
pub const ZAPPED_AFTER_S: f64 = 180.0;
pub const MAX_DURATION_S: f64 = 4.0 * 3600.0;
pub const KEEP_FINAL_S: f64 = 15.0 * 60.0;
const SEEN_KEPT: usize = 500;
const REQUIRED_MAX: usize = 50;

#[derive(Debug, Clone, Default)]
pub struct Pronos {
    pub seen: Vec<String>,
    pub season: BTreeMap<String, Season>,
    pub current: Option<Current>,
    pub cancelled: Option<Current>,
    pub calibration: Calibrage,
    pub direct_key: Option<DirectKey>,
    pub default_delay_s: f64,
    pub margin_s: f64,
    pub dirty: bool,
}
impl Pronos {
    pub fn from_file(file: File, default_delay_s: f64, margin_s: f64) -> Self {
        Self {
            seen: file.seen,
            season: file.season,
            current: file.current,
            calibration: Calibrage::with_persisted(file.calibration),
            default_delay_s,
            margin_s,
            ..Self::default()
        }
    }

    pub fn to_file(&self) -> File {
        File {
            seen: self.seen[self.seen.len().saturating_sub(SEEN_KEPT)..].to_vec(),
            season: self.season.clone(),
            current: self.current.clone(),
            calibration: self.calibration.persisted.clone(),
        }
    }

    pub fn active(&self) -> bool {
        self.current
            .as_ref()
            .is_some_and(|c| c.status != Status::Final)
    }

    pub fn is_seen(&self, id: &str) -> bool {
        self.seen.iter().any(|s| s == id)
    }

    pub fn delay(&mut self, now: f64) -> (f64, &'static str) {
        self.calibration
            .for_match(self.current.as_ref().map(|c| c.id.as_str()));
        self.calibration.delay(
            self.default_delay_s,
            now,
            self.direct_key.as_ref(),
            self.margin_s,
        )
    }

    pub fn measure(&mut self, match_id: &str, delay: f64, now: f64) -> bool {
        if self.current.as_ref().is_none_or(|c| c.id != match_id) {
            return false;
        }
        self.calibration.for_match(Some(match_id));
        let key = self.direct_key.clone();
        let ok = self.calibration.add(delay, now, key.as_ref());
        self.dirty |= ok;
        ok
    }

    pub fn open(
        &mut self,
        found: &FlatMatch,
        channel: &str,
        required: &[(String, String)],
        auto: bool,
        now: f64,
    ) -> bool {
        if self.active() || self.is_seen(&found.id) {
            return false;
        }
        let in_match = found.state == "in";
        let closes_at = if in_match {
            now + 30f64.max((MAX_MINUTE_IN_MATCH - found.minute_n.unwrap_or(0)) as f64 * 60.0)
        } else {
            found.start.unwrap_or(now + BEFORE_KICKOFF_S)
        };
        self.cancelled = None;
        let names: BTreeMap<String, String> = required
            .iter()
            .map(|(id, name)| (id.clone(), name.chars().take(40).collect()))
            .collect();
        self.current = Some(Current {
            id: found.id.clone(),
            code: found.code.clone(),
            competition: found.competition.clone(),
            home: found.home.clone(),
            away: found.away.clone(),
            home_abbr: found.home_abbr.clone(),
            away_abbr: found.away_abbr.clone(),
            home_color: found.home_color.clone(),
            away_color: found.away_color.clone(),
            status: Status::Open,
            closes_at,
            opened_mid_match: in_match,
            kickoff: found.start.unwrap_or(now),
            channel: channel.chars().take(60).collect(),
            score: found.score.clone(),
            minute: found.minute.clone(),
            auto,
            votes: BTreeMap::new(),
            required: required
                .iter()
                .map(|(id, _)| id.clone())
                .take(REQUIRED_MAX)
                .collect(),
            names,
            results: Vec::new(),
            counted: false,
            missed: 0,
            opened_at: now,
            away_since: None,
            ended_at: None,
        });
        self.seen.push(found.id.clone());
        self.dirty = true;
        true
    }

    pub fn vote(
        &mut self,
        user_id: &str,
        name: &str,
        match_id: &str,
        score: &str,
        now: f64,
    ) -> (bool, &'static str) {
        let Some(current) = self.current.as_mut().filter(|c| c.id == match_id) else {
            return (false, "noOpenProno");
        };
        if current.status != Status::Open || now >= current.closes_at {
            return (false, "closed");
        }
        let Some((h, a)) = parse_score(score) else {
            return (false, "invalidScore");
        };
        if user_id.is_empty() {
            return (false, "invalidScore");
        }
        current
            .votes
            .insert(user_id.to_string(), format!("{h}-{a}"));
        current
            .names
            .insert(user_id.to_string(), name.chars().take(40).collect());
        if !current.required.iter().any(|r| r == user_id) && current.required.len() < REQUIRED_MAX {
            current.required.push(user_id.to_string());
        }
        self.dirty = true;
        (true, "")
    }
}

pub fn in_window(found: &FlatMatch, now: f64, before_s: f64) -> bool {
    match found.state.as_str() {
        "pre" => found
            .start
            .is_some_and(|s| (-5.0 * 60.0..=before_s).contains(&(s - now))),
        "in" => found.period == 1 && found.minute_n.is_some_and(|m| m <= MAX_MINUTE_IN_MATCH),
        _ => false,
    }
}
