use std::collections::BTreeMap;

use serde_json::Value;

use super::model::{Player, Teams};
use crate::text::norm_lossy;

pub type PlayerIndex = BTreeMap<String, (String, String)>;

fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn number(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

pub fn players(rosters: &[Value]) -> (Vec<Player>, PlayerIndex) {
    let mut players = Vec::new();
    let mut index = PlayerIndex::new();
    for roster in rosters {
        let side = if text(roster, "homeAway") == "home" {
            "dom"
        } else {
            "ext"
        };
        let lines: Vec<Value> = roster
            .get("roster")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let last_names: Vec<String> = lines
            .iter()
            .map(|l| text(l.get("athlete").unwrap_or(&Value::Null), "lastName"))
            .collect();
        for line in &lines {
            let athlete = line.get("athlete").cloned().unwrap_or(Value::Null);
            let id = match athlete.get("id") {
                Some(Value::String(s)) if !s.is_empty() => s.clone(),
                Some(Value::Number(n)) => n.to_string(),
                _ => continue,
            };
            let mut name = [
                text(&athlete, "lastName"),
                text(&athlete, "shortName"),
                text(&athlete, "displayName"),
            ]
            .into_iter()
            .find(|n| !n.is_empty())
            .unwrap_or_default();
            if !name.is_empty() && last_names.iter().filter(|n| **n == name).count() > 1 {
                name = [text(&athlete, "shortName"), text(&athlete, "displayName")]
                    .into_iter()
                    .find(|n| !n.is_empty())
                    .unwrap_or(name);
            }
            let starter = line
                .get("starter")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let place = if starter {
                number(line.get("formationPlace")).unwrap_or(0.0) as i64
            } else {
                0
            };
            let num = number(line.get("jersey")).unwrap_or(0.0) as i64;
            let position: String =
                text(line.get("position").unwrap_or(&Value::Null), "abbreviation")
                    .chars()
                    .take(6)
                    .collect();
            players.push(Player {
                id: id.clone(),
                side: side.into(),
                num,
                name: name.chars().take(30).collect(),
                position,
                place,
                starter,
                ..Player::default()
            });
            for key in ["displayName", "fullName"] {
                let full = text(&athlete, key);
                if !full.is_empty() {
                    index.insert(norm_lossy(&full), (id.clone(), side.to_string()));
                }
            }
        }
    }
    players.sort_by_key(|p| (p.side != "dom", !p.starter));
    (players, index)
}

fn base_position(position: &str) -> String {
    position
        .to_uppercase()
        .split('-')
        .next()
        .unwrap_or_default()
        .to_string()
}

fn depth(position: &str) -> Option<f64> {
    Some(match base_position(position).as_str() {
        "CD" => 1.0,
        "SW" => 0.8,
        "LB" | "RB" => 1.2,
        "LWB" | "RWB" => 1.8,
        "DM" => 2.0,
        "CM" => 2.5,
        "LM" | "RM" => 2.6,
        "AM" => 3.0,
        "LW" | "RW" | "LF" | "RF" | "SS" => 3.5,
        "F" | "CF" | "ST" => 4.0,
        _ => return None,
    })
}

fn lateral_rank(position: &str) -> i64 {
    let p = position.to_uppercase();
    if p.ends_with("-L") {
        -1
    } else if p.ends_with("-R") {
        1
    } else if p.len() >= 2 && p.chars().all(|c| c.is_ascii_alphabetic()) && p.starts_with('L') {
        -2
    } else if p.len() >= 2 && p.chars().all(|c| c.is_ascii_alphabetic()) && p.starts_with('R') {
        2
    } else {
        0
    }
}

fn formation_rows(formation: &str) -> Option<Vec<usize>> {
    let rows: Option<Vec<usize>> = formation
        .split('-')
        .map(|x| x.trim().parse().ok().filter(|n| *n > 0))
        .collect();
    rows.filter(|r| !r.is_empty() && r.iter().sum::<usize>() == 10)
}

pub fn place(players: &mut [Player], teams: &Teams) {
    for side in ["dom", "ext"] {
        let Some(rows) = formation_rows(&teams.side(side).formation) else {
            continue;
        };
        let starters: Vec<usize> = (0..players.len())
            .filter(|i| players[*i].side == side && players[*i].starter)
            .collect();
        if starters.len() != 11 {
            continue;
        }
        let mut keepers: Vec<usize> = starters
            .iter()
            .copied()
            .filter(|i| base_position(&players[*i].position) == "G")
            .collect();
        if keepers.is_empty() {
            keepers = starters
                .iter()
                .copied()
                .filter(|i| players[*i].place == 1)
                .collect();
        }
        let [keeper] = keepers[..] else { continue };
        let mut field: Vec<usize> = starters.iter().copied().filter(|i| *i != keeper).collect();
        if field.iter().any(|i| depth(&players[*i].position).is_none()) {
            field.sort_by_key(|i| players[*i].place);
        } else {
            field.sort_by(|a, b| {
                depth(&players[*a].position)
                    .unwrap()
                    .total_cmp(&depth(&players[*b].position).unwrap())
                    .then(players[*a].place.cmp(&players[*b].place))
            });
        }
        let total_rows = rows.len() as i64 + 1;
        players[keeper].row = Some(0);
        players[keeper].rows = Some(total_rows);
        players[keeper].x = Some(50.0);
        let mut cursor = 0;
        for (k, count) in rows.iter().enumerate() {
            let mut line: Vec<usize> = field[cursor..cursor + count].to_vec();
            cursor += count;
            line.sort_by_key(|i| (lateral_rank(&players[*i].position), players[*i].place));
            for (r, i) in line.iter().enumerate() {
                players[*i].row = Some(k as i64 + 1);
                players[*i].rows = Some(total_rows);
                players[*i].x =
                    Some((1000.0 * (r as f64 + 1.0) / (*count as f64 + 1.0)).round() / 10.0);
            }
        }
    }
}
