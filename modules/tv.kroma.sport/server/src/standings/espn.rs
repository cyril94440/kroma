use serde_json::Value;

use super::compute::{LiveGame, StandingRow};
use crate::text::norm_lossy;

pub const LEAGUES: [(&str, &str); 14] = [
    ("ligue 1", "fra.1"),
    ("ligue1", "fra.1"),
    ("premier league", "eng.1"),
    ("liga", "esp.1"),
    ("la liga", "esp.1"),
    ("serie a", "ita.1"),
    ("bundesliga", "ger.1"),
    ("ligue 2", "fra.2"),
    ("ligue des champions", "uefa.champions"),
    ("champions league", "uefa.champions"),
    ("ligue europa", "uefa.europa"),
    ("europa league", "uefa.europa"),
    ("conference league", "uefa.europa.conf"),
    ("ligue conference", "uefa.europa.conf"),
];

pub fn competition_code(name: &str) -> Option<&'static str> {
    let folded = norm_lossy(if name.trim().is_empty() {
        "ligue 1"
    } else {
        name
    });
    LEAGUES
        .iter()
        .find(|(k, _)| *k == folded)
        .or_else(|| LEAGUES.iter().find(|(k, _)| folded.contains(k)))
        .map(|(_, code)| *code)
}

fn text(value: &Value, key: &str) -> Value {
    value.get(key).cloned().unwrap_or(Value::Null)
}

pub fn from_espn(table: &Value, scores: &Value) -> (Vec<StandingRow>, Vec<LiveGame>) {
    let groups = table
        .get("children")
        .and_then(Value::as_array)
        .filter(|g| !g.is_empty())
        .cloned()
        .unwrap_or_else(|| vec![table.clone()]);
    let entries = groups[0]
        .get("standings")
        .and_then(|s| s.get("entries"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let rows = entries
        .iter()
        .map(|entry| {
            let stat = |name: &str| {
                entry
                    .get("stats")
                    .and_then(Value::as_array)
                    .and_then(|s| {
                        s.iter()
                            .find(|x| x.get("name").and_then(Value::as_str) == Some(name))
                    })
                    .map_or(Value::Null, |x| text(x, "value"))
            };
            StandingRow {
                club: entry
                    .get("team")
                    .map_or(Value::Null, |t| text(t, "displayName")),
                pts: stat("points"),
                diff: stat("pointDifferential"),
            }
        })
        .collect();
    let games = scores
        .get("events")
        .and_then(Value::as_array)
        .map(|events| {
            events
                .iter()
                .filter_map(|event| {
                    let competition = event.get("competitions")?.get(0)?;
                    let status = competition.get("status")?;
                    let kind = status.get("type")?;
                    if kind.get("state").and_then(Value::as_str) != Some("in") {
                        return None;
                    }
                    let competitors = competition.get("competitors").and_then(Value::as_array)?;
                    let side = |name: &str| {
                        competitors
                            .iter()
                            .find(|c| c.get("homeAway").and_then(Value::as_str) == Some(name))
                    };
                    let (home, away) = (side("home")?, side("away")?);
                    let score = |c: &Value| match c.get("score") {
                        Some(Value::String(s)) => s.clone(),
                        Some(Value::Number(n)) => n.to_string(),
                        _ => "None".into(),
                    };
                    let minute = status
                        .get("displayClock")
                        .and_then(Value::as_str)
                        .filter(|m| !m.is_empty())
                        .map_or_else(|| text(kind, "shortDetail"), |m| Value::String(m.into()));
                    Some(LiveGame {
                        home: home
                            .get("team")
                            .map_or(Value::Null, |t| text(t, "displayName")),
                        away: away
                            .get("team")
                            .map_or(Value::Null, |t| text(t, "displayName")),
                        score: Value::String(format!("{}-{}", score(home), score(away))),
                        minute,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    (rows, games)
}
