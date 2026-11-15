use serde_json::{json, Value};

use crate::detect::{flat_matches, FlatMatch};
use crate::iso::parse_secs;
use crate::prono::{File, Pronos};

pub fn t0() -> f64 {
    parse_secs("2026-10-30T17:50:00Z").unwrap()
}

#[derive(Clone)]
pub struct Game {
    pub id: &'static str,
    pub home: &'static str,
    pub away: &'static str,
    pub state: &'static str,
    pub score: (&'static str, &'static str),
    pub clock: &'static str,
    pub period: i64,
    pub date: &'static str,
    pub abbr: (&'static str, &'static str),
}

pub fn havre() -> Game {
    Game {
        id: "401",
        home: "Le Havre AC",
        away: "Paris Saint-Germain",
        state: "pre",
        score: ("0", "0"),
        clock: "0'",
        period: 0,
        date: "2026-10-30T18:00Z",
        abbr: ("HAC", "PSG"),
    }
}

pub fn lens() -> Game {
    Game {
        id: "402",
        home: "Lens",
        away: "Paris FC",
        abbr: ("RCL", "PFC"),
        ..havre()
    }
}

impl Game {
    pub fn at(self, state: &'static str, clock: &'static str, period: i64) -> Game {
        Game {
            state,
            clock,
            period,
            ..self
        }
    }

    pub fn scored(self, home: &'static str, away: &'static str) -> Game {
        Game {
            score: (home, away),
            ..self
        }
    }

    pub fn on(self, date: &'static str) -> Game {
        Game { date, ..self }
    }

    fn json(&self) -> Value {
        let side = |ha: &str, name: &str, score: &str, abbr: &str, color: &str| json!({"homeAway": ha, "score": score, "team": {"abbreviation": abbr, "displayName": name, "shortDisplayName": name, "name": name, "location": name, "color": color}});
        json!({"id": self.id, "date": self.date, "competitions": [{"date": self.date, "status": {"displayClock": self.clock, "period": self.period, "type": {"state": self.state}},
            "competitors": [side("home", self.home, self.score.0, self.abbr.0, "011F68"), side("away", self.away, self.score.1, self.abbr.1, "1b2a5c")]}]})
    }
}

pub fn matches(games: &[Game]) -> Vec<FlatMatch> {
    flat_matches(
        &json!({"events": games.iter().map(Game::json).collect::<Vec<_>>()}),
        "fra.1",
    )
}

pub fn pronos() -> Pronos {
    Pronos::from_file(File::default(), 60.0, 15.0)
}

pub fn people(names: &[&str]) -> Vec<(String, String)> {
    names
        .iter()
        .map(|n| (format!("u-{}", n.to_lowercase()), n.to_string()))
        .collect()
}

pub fn id(name: &str) -> String {
    format!("u-{}", name.to_lowercase())
}
