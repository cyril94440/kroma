use std::collections::BTreeMap;

use serde::Serialize;

use super::flat::FlatMatch;
use super::image::read_image;
use super::scoring::{rate, DetectProgramme};
use crate::sport::Reading;

pub const PRE_MAX_S: f64 = 30.0 * 60.0;
pub const PRE_LATE_MAX_S: f64 = 20.0 * 60.0;
pub const IN_MAX_S: f64 = 3.0 * 3600.0 + 30.0 * 60.0;
pub const POST_WINDOW_S: f64 = 15.0 * 60.0;
pub const END_AFTER_START_S: f64 = 115.0 * 60.0;
pub const END_AFTER_START_EXTRA_S: f64 = 160.0 * 60.0;
pub const THRESHOLD: f64 = 8.0;
pub const MARGIN: f64 = 4.0;
const LISTED: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Ok,
    Ambiguous,
    Nothing,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Candidate {
    pub id: String,
    #[serde(rename = "match")]
    pub label: String,
    pub competition: String,
    pub score: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Decision {
    pub verdict: Verdict,
    pub matched: Option<FlatMatch>,
    pub score: f64,
    pub candidates: Vec<Candidate>,
    pub reasons: Vec<String>,
}

fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

pub fn candidates(
    matches: &[FlatMatch],
    now: f64,
    post_seen: &BTreeMap<String, f64>,
) -> Vec<FlatMatch> {
    matches
        .iter()
        .filter(|m| match m.state.as_str() {
            "pre" => m
                .start
                .is_some_and(|s| (-PRE_LATE_MAX_S..=PRE_MAX_S).contains(&(s - now))),
            "in" => m
                .start
                .is_none_or(|s| s - now <= 600.0 && now - s <= IN_MAX_S),
            "post" => estimated_end(m, post_seen)
                .is_some_and(|end| (0.0..=POST_WINDOW_S).contains(&(now - end))),
            _ => false,
        })
        .cloned()
        .collect()
}

pub fn estimated_end(m: &FlatMatch, post_seen: &BTreeMap<String, f64>) -> Option<f64> {
    post_seen.get(&m.id).copied().or_else(|| {
        m.start.map(|s| {
            s + if m.period >= 3 {
                END_AFTER_START_EXTRA_S
            } else {
                END_AFTER_START_S
            }
        })
    })
}

pub fn decide(
    matches: &[FlatMatch],
    programmes: &[DetectProgramme],
    now: f64,
    image: Option<&Reading>,
    post_seen: &BTreeMap<String, f64>,
) -> Decision {
    let pool = candidates(matches, now, post_seen);
    let mut rated: Vec<(f64, &FlatMatch, Vec<String>)> = pool
        .iter()
        .map(|m| {
            let (score, reasons) = rate(m, programmes, now, image, &pool);
            (score, m, reasons)
        })
        .collect();
    rated.sort_by(|a, b| {
        b.0.total_cmp(&a.0)
            .then(
                a.1.start
                    .unwrap_or(0.0)
                    .total_cmp(&b.1.start.unwrap_or(0.0)),
            )
            .then(a.1.id.cmp(&b.1.id))
    });
    let nothing = Decision {
        verdict: Verdict::Nothing,
        matched: None,
        score: 0.0,
        candidates: Vec::new(),
        reasons: Vec::new(),
    };
    let Some((best, leader, leader_reasons)) = rated.first() else {
        return nothing;
    };
    if *best <= 0.0 {
        return nothing;
    }
    let listed: Vec<Candidate> = rated
        .iter()
        .take(LISTED)
        .map(|(s, m, _)| Candidate {
            id: m.id.clone(),
            label: m.label(),
            competition: m.competition.clone(),
            score: round1(*s),
        })
        .collect();
    let second = rated.get(1).map_or(-99.0, |r| r.0);
    if *best >= THRESHOLD && best - second >= MARGIN {
        return Decision {
            verdict: Verdict::Ok,
            matched: Some((*leader).clone()),
            score: round1(*best),
            candidates: listed,
            reasons: leader_reasons.clone(),
        };
    }
    let plausible = listed
        .into_iter()
        .filter(|c| c.score >= best - MARGIN && c.score > 0.0)
        .collect();
    let why = if *best < THRESHOLD {
        "score trop faible"
    } else {
        "plusieurs matchs plausibles"
    };
    Decision {
        verdict: Verdict::Ambiguous,
        matched: None,
        score: round1(*best),
        candidates: plausible,
        reasons: vec![format!("pas de vainqueur net : {why}")],
    }
}

pub fn contradicts(found: &FlatMatch, image: Option<&Reading>, pool: &[FlatMatch]) -> bool {
    read_image(found, image, pool)
        .1
        .iter()
        .any(|r| r.contains("d'autres équipes"))
}

#[cfg(test)]
pub fn confirms(found: &FlatMatch, image: Option<&Reading>, pool: &[FlatMatch]) -> bool {
    read_image(found, image, pool)
        .1
        .iter()
        .any(|r| r.contains("équipe") && !r.contains("d'autres"))
}

pub fn merge(lists: &[Vec<FlatMatch>]) -> Vec<FlatMatch> {
    let mut by_id: BTreeMap<String, FlatMatch> = BTreeMap::new();
    for list in lists {
        for m in list {
            by_id.insert(m.id.clone(), m.clone());
        }
    }
    by_id.into_values().collect()
}
