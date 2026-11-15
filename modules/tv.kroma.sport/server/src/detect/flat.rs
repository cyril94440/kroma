use serde_json::Value;

use super::names::{team_forms, Forms, TeamNames};
use crate::iso::parse_secs;

const LABELS: [(&str, &str); 9] = [
    ("fra.1", "Ligue 1"),
    ("eng.1", "Premier League"),
    ("esp.1", "Liga"),
    ("ita.1", "Serie A"),
    ("ger.1", "Bundesliga"),
    ("fra.2", "Ligue 2"),
    ("uefa.champions", "Ligue des champions"),
    ("uefa.europa", "Ligue Europa"),
    ("uefa.europa.conf", "Ligue Conférence"),
];

#[derive(Debug, Clone, PartialEq, Default)]
pub struct FlatMatch {
    pub id: String,
    pub code: String,
    pub competition: String,
    pub venue: String,
    pub city: String,
    pub home: String,
    pub away: String,
    pub home_abbr: String,
    pub away_abbr: String,
    pub home_color: String,
    pub away_color: String,
    pub home_names: [String; 5],
    pub away_names: [String; 5],
    pub state: String,
    pub period: i64,
    pub score: String,
    pub clock_s: Option<f64>,
    pub half_time: bool,
    pub minute: String,
    pub minute_n: Option<i64>,
    pub start: Option<f64>,
}

fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn color(raw: &str) -> String {
    let hex = raw.trim_start_matches('#');
    if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        format!("#{}", hex.to_lowercase())
    } else {
        "#6b6256".into()
    }
}

fn minute_number(clock: &str) -> Option<i64> {
    let digits: String = clock
        .trim_start()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

fn score_number(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::String(s) => s.parse::<f64>().ok().map(|f| f as i64),
        Value::Number(n) => n.as_f64().map(|f| f as i64),
        _ => None,
    }
}

fn names(team: &Value) -> [String; 5] {
    [
        "displayName",
        "shortDisplayName",
        "name",
        "location",
        "abbreviation",
    ]
    .map(|k| text(team, k))
}

fn competition_name(competition: &Value, event: &Value) -> String {
    let note = Some(text(competition, "altGameNote"))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| text(event, "altGameNote"));
    note.split(',')
        .next()
        .unwrap_or_default()
        .trim()
        .chars()
        .take(60)
        .collect()
}

pub fn flat_matches(scores: &Value, code: &str) -> Vec<FlatMatch> {
    let events = scores
        .get("events")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    events
        .iter()
        .filter_map(|event| flat_match(event, code))
        .collect()
}

fn flat_match(event: &Value, code: &str) -> Option<FlatMatch> {
    let competition = event.get("competitions")?.get(0)?;
    let status = competition.get("status").cloned().unwrap_or(Value::Null);
    let competitors = competition.get("competitors")?.as_array()?;
    let side = |name: &str| competitors.iter().find(|c| text(c, "homeAway") == name);
    let (home, away) = (side("home")?, side("away")?);
    let (team_home, team_away) = (home.get("team")?, away.get("team")?);
    let (home_names, away_names) = (names(team_home), names(team_away));
    let id = event
        .get("id")
        .map(|v| v.as_str().map_or_else(|| v.to_string(), str::to_string))?;
    if home_names[0].is_empty() || away_names[0].is_empty() || id.is_empty() {
        return None;
    }
    let kind = status.get("type").cloned().unwrap_or(Value::Null);
    let state = text(&kind, "state");
    let score = if state == "pre" {
        "0-0".to_string()
    } else {
        format!(
            "{}-{}",
            score_number(home.get("score"))?,
            score_number(away.get("score"))?
        )
    };
    let clock = text(&status, "displayClock");
    let venue = competition
        .get("venue")
        .or_else(|| event.get("venue"))
        .cloned()
        .unwrap_or(Value::Null);
    let city = venue
        .get("address")
        .map(|a| text(a, "city"))
        .unwrap_or_default();
    Some(FlatMatch {
        id: format!("espn-{id}"),
        code: code.to_string(),
        competition: LABELS.iter().find(|(c, _)| *c == code).map_or_else(
            || competition_name(competition, event),
            |(_, l)| l.to_string(),
        ),
        venue: text(&venue, "fullName").chars().take(60).collect(),
        city: city.chars().take(40).collect(),
        home: home_names[0].clone(),
        away: away_names[0].clone(),
        home_abbr: home_names[4].chars().take(4).collect(),
        away_abbr: away_names[4].chars().take(4).collect(),
        home_color: color(&text(team_home, "color")),
        away_color: color(&text(team_away, "color")),
        home_names,
        away_names,
        state,
        period: status.get("period").and_then(Value::as_i64).unwrap_or(0),
        score,
        clock_s: status.get("clock").and_then(Value::as_f64),
        half_time: text(&kind, "name").to_uppercase() == "STATUS_HALFTIME",
        minute_n: minute_number(&clock),
        minute: clock.chars().take(8).collect(),
        start: parse_secs(
            &Some(text(competition, "date"))
                .filter(|d| !d.is_empty())
                .unwrap_or_else(|| text(event, "date")),
        ),
    })
}

impl FlatMatch {
    pub fn names(&self, home: bool) -> TeamNames {
        let n = if home {
            &self.home_names
        } else {
            &self.away_names
        };
        let abbreviation = if home {
            &self.home_abbr
        } else {
            &self.away_abbr
        };
        TeamNames {
            display: n[0].clone(),
            short: n[1].clone(),
            name: n[2].clone(),
            location: n[3].clone(),
            abbreviation: abbreviation.clone(),
        }
    }

    pub fn forms(&self, home: bool, with_abbreviation: bool) -> Forms {
        team_forms(&self.names(home), with_abbreviation)
    }

    pub fn slug(&self) -> Option<&'static str> {
        super::competitions::slug_espn(&self.competition)
    }

    pub fn label(&self) -> String {
        format!("{} – {}", self.home, self.away)
    }
}
