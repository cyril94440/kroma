use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use crate::iso::parse_secs;
use crate::live_match::{Fact, MatchPayload};

pub const BEFORE_S: f64 = 60.0;
pub const BEFORE_STOPPAGE_S: f64 = 110.0;
pub const AFTER_S: f64 = 50.0;
pub const FALLBACK_BEFORE_S: f64 = 95.0;
pub const FALLBACK_AFTER_S: f64 = 70.0;
pub const CLOSING_S: f64 = 10.0;
pub const USEFUL_BUFFER_S: f64 = 560.0;
pub const ATTEMPTS_MAX: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Window {
    pub from_s: f64,
    pub to_s: f64,
    pub pivot_s: f64,
    pub from_image: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalState {
    ToCut,
    Done,
    GaveUp,
    NoTime,
    TooEarly,
    OutOfBuffer,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Label {
    pub match_id: String,
    pub ev_id: String,
    pub dom: String,
    pub ext: String,
    pub competition: String,
    pub minute: String,
    pub equipe: String,
    pub buteur: String,
    pub score: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GoalReview {
    pub id: String,
    pub state: GoalState,
    pub window: Option<Window>,
    pub label: Label,
    pub period: i64,
    pub t_image: Option<i64>,
    pub t_vu: i64,
    pub espn_time_s: Option<f64>,
    pub period_calibrated: bool,
}

pub fn real_times(summary: &Value) -> BTreeMap<String, f64> {
    let events = summary
        .get("keyEvents")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    events
        .iter()
        .filter_map(|k| {
            let at = parse_secs(k.get("wallclock")?.as_str()?)?;
            let id = match k.get("id")? {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                _ => return None,
            };
            Some((id, at))
        })
        .collect()
}

pub fn clip_id(match_id: &str, fact_id: &str) -> String {
    let raw = format!("{match_id}-but-{fact_id}").to_lowercase();
    let mut out = String::new();
    for c in raw.chars() {
        let keep = c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-';
        if keep {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').chars().take(80).collect()
}

pub fn window(
    fact: &Fact,
    calibrated: bool,
    real: &BTreeMap<String, f64>,
    delay_s: f64,
) -> Option<Window> {
    if let (true, Some(t_image)) = (calibrated, fact.t_image) {
        let t = t_image as f64 / 1000.0;
        let before = if fact.minute.contains('+') {
            BEFORE_STOPPAGE_S
        } else {
            BEFORE_S
        };
        return Some(Window {
            from_s: t - before,
            to_s: t + AFTER_S,
            pivot_s: t - 10.0,
            from_image: true,
        });
    }
    let t = real.get(&fact.id)? + delay_s;
    Some(Window {
        from_s: t - FALLBACK_BEFORE_S,
        to_s: t + FALLBACK_AFTER_S,
        pivot_s: t,
        from_image: false,
    })
}

fn label(m: &MatchPayload, fact: &Fact) -> Label {
    let (mut dom, mut ext) = (0, 0);
    let mut goals: Vec<&Fact> = m.facts.iter().filter(|f| f.kind == "but").collect();
    goals.sort_by_key(|f| (f.period, f.s));
    for goal in goals {
        match goal.side.as_str() {
            "dom" => dom += 1,
            "ext" => ext += 1,
            _ => {}
        }
        if goal.id == fact.id {
            break;
        }
    }
    Label {
        match_id: m.id.clone(),
        ev_id: fact.id.clone(),
        dom: m.teams.dom.name.clone(),
        ext: m.teams.ext.name.clone(),
        competition: m.competition.clone(),
        minute: fact.minute.clone(),
        equipe: fact.side.clone(),
        buteur: fact
            .player
            .as_ref()
            .and_then(|id| m.players.iter().find(|p| &p.id == id))
            .map(|p| p.name.clone())
            .unwrap_or_default(),
        score: format!("{dom}-{ext}"),
    }
}

pub fn review(
    m: &MatchPayload,
    summary: &Value,
    now_s: f64,
    delay_s: f64,
    done: &BTreeMap<String, f64>,
    attempts: &BTreeMap<String, u32>,
    buffer_from_s: Option<f64>,
) -> Vec<GoalReview> {
    if !matches!(m.state.as_str(), "in" | "post") {
        return Vec::new();
    }
    let calibrated: BTreeMap<i64, bool> = m
        .periods
        .iter()
        .map(|p| (p.number, p.t_image.is_some()))
        .collect();
    let real = real_times(summary);
    m.facts
        .iter()
        .filter(|f| f.kind == "but")
        .map(|fact| {
            let id = clip_id(&m.id, &fact.id);
            let period_calibrated = calibrated.get(&fact.period).copied().unwrap_or(false);
            let win = window(fact, period_calibrated, &real, delay_s);
            let useful_from = buffer_from_s.unwrap_or(now_s - USEFUL_BUFFER_S);
            let state = if done.contains_key(&id) {
                GoalState::Done
            } else if attempts.get(&id).copied().unwrap_or(0) >= ATTEMPTS_MAX {
                GoalState::GaveUp
            } else if let Some(w) = win {
                if now_s < w.to_s + CLOSING_S {
                    GoalState::TooEarly
                } else if w.from_s < useful_from {
                    GoalState::OutOfBuffer
                } else {
                    GoalState::ToCut
                }
            } else {
                GoalState::NoTime
            };
            GoalReview {
                id,
                state,
                window: win,
                label: label(m, fact),
                period: fact.period,
                t_image: fact.t_image,
                t_vu: fact.t_vu,
                espn_time_s: real.get(&fact.id).copied(),
                period_calibrated,
            }
        })
        .collect()
}

pub fn to_cut(reviews: Vec<GoalReview>) -> Vec<GoalReview> {
    reviews
        .into_iter()
        .filter(|r| r.state == GoalState::ToCut)
        .collect()
}
