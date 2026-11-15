use serde_json::Value;

use super::event::{Line, Sport, SportEvent, State};
use crate::iso::parse_secs;
use crate::text::fold;

const FRENCH_CLUBS: [&str; 15] = [
    "paris saint germain",
    "psg",
    "marseille",
    "lyon",
    "monaco",
    "lille",
    "lens",
    "nice",
    "rennes",
    "strasbourg",
    "nantes",
    "toulouse",
    "brest",
    "reims",
    "france",
];

pub(super) fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

pub(super) fn state_of(status: &Value) -> Option<State> {
    let kind = status.get("type")?;
    let state = kind.get("state").and_then(Value::as_str)?;
    if state == "post"
        && !kind
            .get("completed")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        return None;
    }
    State::parse(state)
}

fn clock_text(status: &Value) -> String {
    let kind = status.get("type").cloned().unwrap_or(Value::Null);
    if text(&kind, "name") == "STATUS_HALFTIME" {
        return "Mi-temps".into();
    }
    let display = text(status, "displayClock");
    if display.is_empty() {
        text(&kind, "shortDetail")
    } else {
        display
    }
}

fn score_text(competitor: &Value) -> String {
    match competitor.get("score") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

pub fn soccer(payload: &Value, code: &str, name: &str, rank: i32) -> Vec<SportEvent> {
    let events = payload
        .get("events")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    events
        .iter()
        .filter_map(|event| one(event, code, name, rank))
        .collect()
}

fn one(event: &Value, code: &str, name: &str, rank: i32) -> Option<SportEvent> {
    let competition = event
        .get("competitions")
        .and_then(|c| c.get(0))
        .cloned()
        .unwrap_or(Value::Null);
    let status = competition
        .get("status")
        .or_else(|| event.get("status"))
        .cloned()
        .unwrap_or(Value::Null);
    let state = state_of(&status)?;
    let mut competitors: Vec<Value> = competition.get("competitors")?.as_array()?.clone();
    if competitors.len() != 2 {
        return None;
    }
    competitors.sort_by_key(|c| text(c, "homeAway") != "home");
    let names: Vec<String> = competitors
        .iter()
        .map(|c| text(c.get("team").unwrap_or(&Value::Null), "displayName"))
        .collect();
    let start = parse_secs(&text(event, "date"))?;
    if names.iter().any(String::is_empty) {
        return None;
    }
    let french = names
        .iter()
        .any(|n| FRENCH_CLUBS.contains(&fold(n).as_str()));
    let id = text(event, "id");
    Some(SportEvent {
        id: format!("foot:{id}"),
        espn: id,
        sport: Sport::Soccer,
        code: code.to_string(),
        competition: name.to_string(),
        state,
        start,
        end: None,
        popularity: rank + if french { 20 } else { 0 },
        lines: names
            .iter()
            .zip(&competitors)
            .map(|(n, c)| Line {
                name: n.clone(),
                score: if state == State::Pre {
                    String::new()
                } else {
                    score_text(c)
                },
            })
            .collect(),
        abbreviations: competitors
            .iter()
            .map(|c| text(c.get("team").unwrap_or(&Value::Null), "abbreviation"))
            .collect(),
        clock: if state == State::In {
            clock_text(&status)
        } else {
            String::new()
        },
        tournament: None,
    })
}
