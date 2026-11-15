use std::collections::BTreeMap;

use serde_json::Value;

use crate::detect::{flat_matches, DetectProgramme, FlatMatch};
use crate::iso::parse_secs;
use crate::sport::Reading;
use crate::text::norm_lossy;

pub fn at(clock: &str, day: u32) -> f64 {
    parse_secs(&format!("2026-11-{day:02}T{clock}:00Z")).unwrap()
}

pub fn scoreboard() -> Value {
    serde_json::from_str(include_str!(
        "../../../fixtures/espn_all_scoreboard_20261103.json"
    ))
    .unwrap()
}

pub fn matches_at(now: f64) -> Vec<FlatMatch> {
    let mut board = scoreboard();
    for event in board["events"].as_array_mut().unwrap() {
        let start = parse_secs(event["date"].as_str().unwrap()).unwrap();
        let state = if now < start {
            "pre"
        } else if now < start + 115.0 * 60.0 {
            "in"
        } else {
            "post"
        };
        let competition = &mut event["competitions"][0];
        competition["status"]["type"]["state"] = state.into();
        competition["status"]["period"] = (if state != "post" && now - start < 47.0 * 60.0 {
            1
        } else {
            2
        })
        .into();
        for competitor in competition["competitors"].as_array_mut().unwrap() {
            if competitor["score"].as_str().is_none_or(str::is_empty) {
                competitor["score"] = "0".into();
            }
        }
    }
    flat_matches(&board, "all")
}

pub fn guide(channel: &str) -> Vec<DetectProgramme> {
    let epg: BTreeMap<String, Vec<(f64, f64, String, String)>> =
        serde_json::from_str(include_str!("../../../fixtures/epg_sport_20261103.json")).unwrap();
    epg[channel]
        .iter()
        .map(|(start, stop, title, hay)| DetectProgramme {
            title: title.clone(),
            hay: hay.clone(),
            start: Some(*start),
            stop: Some(*stop),
        })
        .collect()
}

pub fn programme(start: f64, stop: f64, title: &str, desc: &str) -> DetectProgramme {
    DetectProgramme {
        title: title.into(),
        hay: norm_lossy(&format!("{title} {desc}")),
        start: Some(start),
        stop: Some(stop),
    }
}

pub fn synth(
    id: &str,
    home: &str,
    away: &str,
    abbreviations: (&str, &str),
    competition: &str,
    start: f64,
) -> FlatMatch {
    let names = |n: &str| {
        [
            n.to_string(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        ]
    };
    FlatMatch {
        id: id.into(),
        home: home.into(),
        away: away.into(),
        home_names: names(home),
        away_names: names(away),
        home_abbr: abbreviations.0.into(),
        away_abbr: abbreviations.1.into(),
        competition: competition.into(),
        state: "in".into(),
        period: 1,
        score: "0-0".into(),
        start: Some(start),
        ..FlatMatch::default()
    }
}

pub fn direct(overrides: impl FnOnce(&mut Reading)) -> Reading {
    let mut reading = Reading {
        kind: "direct".into(),
        confidence: 0.9,
        ..Reading::default()
    };
    overrides(&mut reading);
    reading
}

pub fn label(decision: &crate::detect::Decision) -> Option<String> {
    decision.matched.as_ref().map(FlatMatch::label)
}

pub fn named<'a>(matches: &'a [FlatMatch], home: &str) -> &'a FlatMatch {
    matches.iter().find(|m| m.home == home).unwrap()
}

pub fn none() -> BTreeMap<String, f64> {
    BTreeMap::new()
}

pub fn score(m: &FlatMatch, now: f64, image: Option<&Reading>, pool: &[FlatMatch]) -> f64 {
    crate::detect::scoring::rate(m, &[], now, image, pool).0
}
