use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;

use super::event::{Line, Sport, SportEvent, State};
use super::soccer::{state_of, text};
use crate::iso::parse_secs;
use crate::text::fold;

const WTA_MAJORS: [&str; 17] = [
    "australian open",
    "roland garros",
    "french open",
    "wimbledon",
    "us open",
    "wta finals",
    "indian wells",
    "miami",
    "madrid",
    "rome",
    "italian",
    "canadian",
    "cincinnati",
    "beijing",
    "wuhan",
    "dubai",
    "doha",
];
const TENNIS_UPCOMING_MAX: usize = 4;

static SPONSORS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(rolex|prudential|msc cruises|the|presented by \w+)\b").unwrap()
});

fn tournament(name: &str) -> String {
    SPONSORS.replace_all(name, "").trim().to_string()
}

fn items<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn athlete(competitor: &Value) -> String {
    competitor
        .get("athlete")
        .map(|a| text(a, "displayName"))
        .unwrap_or_default()
}

pub fn tennis(payload: &Value, circuit: &str, now: f64) -> Vec<SportEvent> {
    let mut out = Vec::new();
    for event in items(payload, "events") {
        let name = tournament(&text(event, "name"));
        let major = event.get("major").and_then(Value::as_bool).unwrap_or(false);
        if circuit == "wta" && !major && !WTA_MAJORS.iter().any(|g| fold(&name).contains(g)) {
            continue;
        }
        let mut upcoming = 0;
        let mut matches: Vec<&Value> = items(event, "groupings")
            .iter()
            .filter(|g| {
                fold(&text(
                    g.get("grouping").unwrap_or(&Value::Null),
                    "displayName",
                ))
                .contains("singles")
            })
            .flat_map(|g| items(g, "competitions"))
            .collect();
        matches.sort_by_key(|c| text(c, "date"));
        for competition in matches {
            let status = competition.get("status").cloned().unwrap_or(Value::Null);
            let (Some(state), Some(start)) =
                (state_of(&status), parse_secs(&text(competition, "date")))
            else {
                continue;
            };
            let round = text(
                competition.get("round").unwrap_or(&Value::Null),
                "displayName",
            );
            let competitors = items(competition, "competitors");
            let unnamed = competitors
                .iter()
                .any(|c| matches!(athlete(c).as_str(), "" | "TBD"));
            let too_far =
                state == State::Pre && (start > now + 86_400.0 || upcoming >= TENNIS_UPCOMING_MAX);
            if state == State::Post
                || competitors.len() != 2
                || round.to_lowercase().contains("qualif")
                || too_far
                || unnamed
            {
                continue;
            }
            if state == State::Pre {
                upcoming += 1;
            }
            let lines = competitors
                .iter()
                .map(|c| Line {
                    name: Some(athlete(c))
                        .filter(|n| !n.is_empty())
                        .unwrap_or_else(|| "?".into()),
                    score: items(c, "linescores")
                        .iter()
                        .filter_map(|s| s.get("value").and_then(Value::as_f64))
                        .map(|v| (v as i64).to_string())
                        .collect::<Vec<_>>()
                        .join(" "),
                })
                .collect();
            let id = text(competition, "id");
            out.push(SportEvent {
                id: format!("tennis:{id}"),
                espn: id,
                sport: Sport::Tennis,
                code: circuit.to_string(),
                competition: if round.is_empty() {
                    name.clone()
                } else {
                    format!("{name} · {round}")
                },
                state,
                start,
                end: None,
                popularity: if state == State::In { 62 } else { 40 },
                lines,
                abbreviations: Vec::new(),
                clock: if state == State::In {
                    text(status.get("type").unwrap_or(&Value::Null), "shortDetail")
                } else {
                    String::new()
                },
                tournament: Some(name.clone()),
            });
        }
    }
    out
}

fn session_label(abbreviation: &str) -> String {
    match abbreviation {
        "FP1" => "Essais 1",
        "FP2" => "Essais 2",
        "FP3" => "Essais 3",
        "Qual" => "Qualifications",
        "SQ" | "SS" => "Qualif. sprint",
        "SR" | "Sprint" => "Sprint",
        "Race" => "Course",
        other => other,
    }
    .to_string()
}

pub fn f1(payload: &Value, now: f64) -> Vec<SportEvent> {
    let mut out = Vec::new();
    for event in items(payload, "events") {
        let name = tournament(&text(event, "name"));
        for competition in items(event, "competitions") {
            let status = competition.get("status").cloned().unwrap_or(Value::Null);
            let (Some(state), Some(start)) =
                (state_of(&status), parse_secs(&text(competition, "date")))
            else {
                continue;
            };
            let abbreviation = text(
                competition.get("type").unwrap_or(&Value::Null),
                "abbreviation",
            );
            if state == State::Post || start > now + 3.0 * 86_400.0 {
                continue;
            }
            let race = abbreviation == "Race";
            let id = Some(text(competition, "id"))
                .filter(|i| !i.is_empty())
                .unwrap_or_else(|| text(event, "id"));
            out.push(SportEvent {
                id: format!("f1:{id}:{abbreviation}"),
                espn: text(event, "id"),
                sport: Sport::F1,
                code: "f1".into(),
                competition: session_label(&abbreviation),
                state,
                start,
                end: None,
                popularity: (if race { 90 } else { 55 }) + if state == State::In { 20 } else { 0 },
                lines: vec![Line {
                    name: name.clone(),
                    score: String::new(),
                }],
                abbreviations: Vec::new(),
                clock: if state == State::In {
                    "En cours".into()
                } else {
                    String::new()
                },
                tournament: Some(name.clone()),
            });
        }
    }
    out
}

pub fn golf(payload: &Value) -> Vec<SportEvent> {
    let mut out = Vec::new();
    for event in items(payload, "events") {
        let status = event.get("status").cloned().unwrap_or(Value::Null);
        let (Some(state), Some(start)) = (state_of(&status), parse_secs(&text(event, "date")))
        else {
            continue;
        };
        if state == State::Post {
            continue;
        }
        let competition = event
            .get("competitions")
            .and_then(|c| c.get(0))
            .cloned()
            .unwrap_or(Value::Null);
        let leaders: Vec<Line> = if state == State::In {
            items(&competition, "competitors")
                .iter()
                .take(2)
                .map(|c| Line {
                    name: Some(athlete(c))
                        .filter(|n| !n.is_empty())
                        .unwrap_or_else(|| "?".into()),
                    score: match c.get("score") {
                        Some(Value::String(s)) => s.clone(),
                        Some(Value::Number(n)) => n.to_string(),
                        _ => String::new(),
                    },
                })
                .collect()
        } else {
            Vec::new()
        };
        let name = tournament(&text(event, "name"));
        let id = text(event, "id");
        out.push(SportEvent {
            id: format!("golf:{id}"),
            espn: id,
            sport: Sport::Golf,
            code: "pga".into(),
            competition: name.clone(),
            state,
            start,
            end: parse_secs(&text(event, "endDate")),
            popularity: if state == State::In { 45 } else { 30 },
            lines: if leaders.is_empty() {
                vec![Line {
                    name: name.clone(),
                    score: String::new(),
                }]
            } else {
                leaders
            },
            abbreviations: Vec::new(),
            clock: if state == State::In {
                text(status.get("type").unwrap_or(&Value::Null), "shortDetail")
            } else {
                String::new()
            },
            tournament: Some(name.clone()),
        });
    }
    out
}
