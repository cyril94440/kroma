use std::sync::LazyLock;

use regex::Regex;
use serde_json::{json, Map, Value};

use crate::detect::{name_designates, FlatMatch, END_AFTER_START_S};
use crate::iso::parse_secs;
use crate::team_match::side_matches;

static SEPARATOR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\s*(?:\s-\s|–|—|/|\bvs\.?\b|\bcontre\b)\s*").unwrap());
static SCORE_TEXT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([^,.!]+?) (\d+), ([^,.!]+?) (\d+)\.").unwrap());
static ADDED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\+.*").unwrap());
static LEADING_DIGITS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(\d+)").unwrap());

pub fn requested_teams(teams: &Value) -> Vec<String> {
    let pieces: Vec<String> = match teams {
        Value::Array(list) => list
            .iter()
            .map(|v| v.as_str().map_or_else(|| v.to_string(), str::to_string))
            .collect(),
        other => SEPARATOR
            .split(other.as_str().unwrap_or_default())
            .map(str::to_string)
            .collect(),
    };
    pieces
        .into_iter()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .take(2)
        .collect()
}

pub fn search<'a>(matches: &'a [FlatMatch], teams: &Value) -> Vec<&'a FlatMatch> {
    let names = requested_teams(teams);
    if names.is_empty() {
        return Vec::new();
    }
    matches
        .iter()
        .filter(|m| {
            names.iter().all(|n| {
                name_designates(n, &m.forms(true, true), &[])
                    || name_designates(n, &m.forms(false, true), &[])
                    || side_matches(n, &m.home_names)
                    || side_matches(n, &m.away_names)
            })
        })
        .collect()
}

pub fn list_line(m: &FlatMatch, now_s: f64, delay_s: f64) -> Value {
    let behind = m.state == "post" && m.start.is_some_and(|start| now_s < start + END_AFTER_START_S + delay_s);
    let phase = if behind { "in" } else { m.state.as_str() };
    let state = match phase {
        "pre" => "a_venir",
        "in" => "en_cours",
        "post" => "termine",
        other => other,
    };
    let mut line = Map::from_iter([
        ("match".to_string(), json!(m.label())),
        ("competition".to_string(), json!(m.competition)),
        ("statut".to_string(), json!(state)),
    ]);
    match phase {
        "post" => {
            line.insert("score".into(), json!(m.score));
        }
        "in" => {
            line.insert(
                "score".into(),
                json!("précise les équipes pour avoir le score (décalé de l'image)"),
            );
        }
        _ => {}
    }
    Value::Object(line)
}

fn approximate_minute(clock: &str, delay_s: f64) -> String {
    if clock.contains('+') {
        return ADDED.replace(clock, "+").trim().to_string();
    }
    LEADING_DIGITS
        .captures(clock)
        .map_or_else(String::new, |m| {
            format!(
                "~{}'",
                (m[1].parse::<i64>().unwrap_or(0) - ((delay_s + 59.0) / 60.0).floor() as i64)
                    .max(1)
            )
        })
}

fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn card_label(kind: &str) -> Option<&'static str> {
    match kind {
        "yellow-card" => Some("jaune"),
        "red-card" => Some("rouge"),
        "yellow-red-card" => Some("rouge (2e jaune)"),
        _ => None,
    }
}

pub fn facts(summary: &Value, now_s: f64, delay_s: f64) -> Value {
    let unreadable = || json!({"error": "réponse ESPN illisible : ne devine pas, dis que tu n'as pas pu la lire"});
    let Some(competition) = summary
        .get("header")
        .and_then(|h| h.get("competitions"))
        .and_then(|c| c.get(0))
    else {
        return unreadable();
    };
    let state = competition
        .get("status")
        .and_then(|s| s.get("type"))
        .map(|t| text(t, "state"))
        .unwrap_or_default();
    let competitors = competition
        .get("competitors")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let team = |side: &str| {
        competitors
            .iter()
            .find(|c| text(c, "homeAway") == side)
            .and_then(|c| c.get("team"))
            .map(|t| text(t, "displayName"))
    };
    let (Some(home), Some(away)) = (team("home"), team("away")) else {
        return unreadable();
    };
    let (home, away) = (
        Some(home)
            .filter(|h| !h.is_empty())
            .unwrap_or_else(|| "?".into()),
        Some(away)
            .filter(|h| !h.is_empty())
            .unwrap_or_else(|| "?".into()),
    );
    let cut = now_s - delay_s;
    let (mut visible, mut hidden) = (Vec::new(), false);
    for event in summary
        .get("keyEvents")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
    {
        let at = parse_secs(&text(&event, "wallclock"));
        if state != "pre" && (at.is_some_and(|t| t > cut) || (at.is_none() && state == "in")) {
            hidden = true;
        } else {
            visible.push(event);
        }
    }
    let status = match state.as_str() {
        "pre" => "a_venir",
        "post" if hidden => "en_cours",
        "post" => "termine",
        _ => "en_cours",
    };
    let (mut goals, mut cards, mut score) = (Vec::new(), Vec::new(), (0, 0));
    for event in &visible {
        let kind = event
            .get("type")
            .map(|t| text(t, "type"))
            .unwrap_or_default();
        let minute = event
            .get("clock")
            .map(|c| text(c, "displayValue"))
            .unwrap_or_default();
        let side = event
            .get("team")
            .map(|t| text(t, "displayName"))
            .unwrap_or_default();
        let players: Vec<String> = event
            .get("participants")
            .and_then(Value::as_array)
            .map(|p| {
                p.iter()
                    .map(|x| {
                        x.get("athlete")
                            .map(|a| text(a, "displayName"))
                            .unwrap_or_default()
                    })
                    .collect()
            })
            .unwrap_or_default();
        let scoring = event
            .get("scoringPlay")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if scoring
            || kind.starts_with("goal")
            || kind.starts_with("penalty---scored")
            || kind == "own-goal"
        {
            let mut goal = Map::from_iter([
                ("minute".to_string(), json!(minute)),
                ("equipe".to_string(), json!(side)),
                (
                    "buteur".to_string(),
                    json!(players.first().filter(|p| !p.is_empty())),
                ),
            ]);
            if kind == "own-goal" {
                goal.insert("type".into(), json!("csc"));
            } else if kind.starts_with("penalty") {
                goal.insert("type".into(), json!("penalty"));
            }
            if let Some(assist) = players.get(1).filter(|p| !p.is_empty()) {
                goal.insert("passeur".into(), json!(assist));
            }
            goals.push(Value::Object(goal));
            score = match SCORE_TEXT.captures(&text(event, "text")) {
                Some(m) => (m[2].parse().unwrap_or(0), m[4].parse().unwrap_or(0)),
                None => (
                    score.0 + i32::from(side == home),
                    score.1 + i32::from(side == away),
                ),
            };
        } else if let (Some(label), Some(player)) =
            (card_label(&kind), players.first().filter(|p| !p.is_empty()))
        {
            cards
                .push(json!({"minute": minute, "equipe": side, "joueur": player, "carton": label}));
        }
    }
    let season = summary
        .get("header")
        .and_then(|h| h.get("season"))
        .map(|s| text(s, "name"))
        .unwrap_or_default();
    let mut out = Map::from_iter([
        ("match".to_string(), json!(format!("{home} – {away}"))),
        ("statut".to_string(), json!(status)),
        ("source".to_string(), json!("espn")),
        ("competition".to_string(), json!(season)),
    ]);
    if status != "a_venir" {
        out.insert("score".into(), json!(format!("{}-{}", score.0, score.1)));
        out.insert("buts".into(), json!(goals));
        out.insert("cartons".into(), json!(cards));
    }
    let competition_status = competition.get("status").cloned().unwrap_or(Value::Null);
    match status {
        "en_cours" => {
            out.insert(
                "minute".into(),
                json!(if state == "in" {
                    approximate_minute(&text(&competition_status, "displayClock"), delay_s)
                } else {
                    String::new()
                }),
            );
            out.insert("note".into(), json!(format!("données décalées de ~{} s pour ne pas devancer l'image ; ne donne pas de fait plus récent que ceux-ci", delay_s as i64)));
        }
        "termine" => {
            let detail = competition_status
                .get("type")
                .map(|t| text(t, "shortDetail"))
                .filter(|d| !d.is_empty())
                .unwrap_or_else(|| "FT".into());
            out.insert("minute".into(), json!(detail));
        }
        _ => {
            out.insert("debut".into(), json!(text(competition, "date")));
        }
    }
    Value::Object(out)
}

#[cfg(test)]
mod tests;
