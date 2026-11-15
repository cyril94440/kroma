use std::collections::BTreeSet;
use std::sync::LazyLock;

use regex::Regex;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::text::norm_lossy;

const EMPTY_WORDS: [&str; 18] = [
    "fc",
    "sc",
    "ac",
    "as",
    "rc",
    "ogc",
    "losc",
    "club",
    "football",
    "stade",
    "olympique",
    "de",
    "du",
    "la",
    "le",
    "les",
    "sporting",
    "us",
];
const MORE_EMPTY: [&str; 5] = ["es", "sco", "afc", "en", "avant"];
const ALIASES: [(&str, &str); 10] = [
    ("psg", "paris saint germain"),
    ("om", "marseille"),
    ("ol", "lyonnais"),
    ("lyon", "lyonnais"),
    ("rennes", "rennais"),
    ("losc", "lille"),
    ("ogcn", "nice"),
    ("hac", "havre"),
    ("pfc", "paris fc"),
    ("paris", "paris saint germain"),
];
const TOP_MAX: usize = 12;
const MOVE_MAX: i64 = 20;

static SCORE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(\d{1,2})\s*[-–:]\s*(\d{1,2})\s*$").unwrap());

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct StandingRow {
    pub club: Value,
    pub pts: Value,
    pub diff: Value,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct LiveGame {
    pub home: Value,
    pub away: Value,
    pub score: Value,
    pub minute: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Provisional {
    pub json: Value,
}

fn as_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn as_int(value: &Value) -> i64 {
    match value {
        Value::Number(n) => n.as_f64().map_or(0, |f| f as i64),
        Value::String(s) => s.trim().parse::<f64>().map_or(0, |f| f as i64),
        _ => 0,
    }
}

pub fn norm_club(name: &str) -> String {
    let folded = norm_lossy(name);
    let folded = ALIASES
        .iter()
        .find(|(from, _)| *from == folded)
        .map_or(folded.clone(), |(_, to)| (*to).to_string());
    let kept: Vec<&str> = folded
        .split(' ')
        .filter(|w| !EMPTY_WORDS.contains(w) && !MORE_EMPTY.contains(w))
        .collect();
    if kept.is_empty() {
        folded
    } else {
        kept.join(" ")
    }
}

fn match_club(name: &str, index: &[(String, usize)]) -> Option<usize> {
    let n = norm_club(name);
    if let Some((_, i)) = index.iter().find(|(k, _)| *k == n) {
        return Some(*i);
    }
    let words: BTreeSet<&str> = n.split(' ').filter(|w| !w.is_empty()).collect();
    let found: BTreeSet<usize> = index
        .iter()
        .filter(|(k, _)| {
            let theirs: BTreeSet<&str> = k.split(' ').collect();
            !words.is_empty() && (words.is_subset(&theirs) || theirs.is_subset(&words))
        })
        .map(|(_, i)| *i)
        .collect();
    (found.len() == 1).then(|| *found.iter().next().unwrap())
}

struct Club {
    name: String,
    pts: i64,
    diff: i64,
    pts0: i64,
    diff0: i64,
    game: String,
    outcome: Option<&'static str>,
}

pub fn provisional(
    rows: &[StandingRow],
    games: &[LiveGame],
    competition: &str,
    top: i64,
) -> Result<Provisional, String> {
    let mut clubs: Vec<Club> = Vec::new();
    let mut index: Vec<(String, usize)> = Vec::new();
    for row in rows {
        let name = as_text(&row.club).trim().to_string();
        if name.is_empty() {
            continue;
        }
        let (pts, diff) = (as_int(&row.pts), as_int(&row.diff));
        if !index.iter().any(|(k, _)| *k == norm_club(&name)) {
            index.push((norm_club(&name), clubs.len()));
        }
        clubs.push(Club {
            name: name.chars().take(40).collect(),
            pts,
            diff,
            pts0: pts,
            diff0: diff,
            game: String::new(),
            outcome: None,
        });
    }
    if clubs.is_empty() {
        return Err(
            "classement de départ vide : cherche d'abord le classement officiel (club, pts, diff)"
                .into(),
        );
    }
    let (mut counted, mut unmatched, mut unreadable) = (0, BTreeSet::new(), Vec::new());
    for game in games {
        let raw_score = as_text(&game.score);
        let Some(score) = SCORE.captures(&raw_score) else {
            unreadable.push(format!(
                "{} – {} : score « {} » illisible",
                as_text(&game.home),
                as_text(&game.away),
                as_text(&game.score)
            ));
            continue;
        };
        let (a, b): (i64, i64) = (score[1].parse().unwrap_or(0), score[2].parse().unwrap_or(0));
        let home = match_club(&as_text(&game.home), &index);
        let away = match_club(&as_text(&game.away), &index);
        for (name, found) in [(&game.home, home), (&game.away, away)] {
            if found.is_none() {
                unmatched.insert(as_text(name));
            }
        }
        let minute: String = as_text(&game.minute).trim().chars().take(8).collect();
        for (found, goals_for, goals_against) in [(home, a, b), (away, b, a)] {
            let Some(i) = found else { continue };
            let club = &mut clubs[i];
            club.pts += if goals_for > goals_against {
                3
            } else {
                i64::from(goals_for == goals_against)
            };
            club.diff += goals_for - goals_against;
            club.game = format!(
                "{goals_for}-{goals_against}{}",
                if minute.is_empty() {
                    String::new()
                } else {
                    format!(" {minute}")
                }
            );
            club.outcome = Some(if goals_for > goals_against {
                "win"
            } else if goals_for == goals_against {
                "draw"
            } else {
                "loss"
            });
        }
        counted += 1;
    }
    let mut official: Vec<usize> = (0..clubs.len()).collect();
    official.sort_by_key(|i| (-clubs[*i].pts0, -clubs[*i].diff0));
    let mut ordered: Vec<usize> = (0..clubs.len()).collect();
    ordered.sort_by_key(|i| (-clubs[*i].pts, -clubs[*i].diff));
    let top = top.clamp(1, TOP_MAX as i64) as usize;
    let rank_in = |list: &[usize], i: usize| {
        list.iter()
            .position(|x| *x == i)
            .map_or(0, |p| p as i64 + 1)
    };
    let table: Vec<Value> = ordered
        .iter()
        .take(top)
        .enumerate()
        .map(|(r, i)| {
            json!([
                (r + 1).to_string(),
                clubs[*i].name,
                clubs[*i].pts.to_string(),
                format!("{:+}", clubs[*i].diff),
                clubs[*i].game
            ])
        })
        .collect();
    let live: Vec<Value> = ordered.iter().take(top).enumerate().map(|(r, i)| json!({"live": clubs[*i].outcome.is_some(), "outcome": clubs[*i].outcome, "move": (rank_in(&official, *i) - (r as i64 + 1)).clamp(-MOVE_MAX, MOVE_MAX)})).collect();
    let title: String = if competition.is_empty() {
        "Classement provisoire".into()
    } else {
        format!("{competition} : classement provisoire")
    }
    .chars()
    .take(60)
    .collect();
    let note: String = if counted > 0 {
        format!("En rouge : points provisoires ({counted} en cours) · ~30 s d'avance sur l'image")
    } else {
        "aucun match en cours compté : classement officiel".into()
    }
    .chars()
    .take(80)
    .collect();
    let mut out = json!({
        "lignes": ordered.iter().enumerate().map(|(r, i)| json!({"rang": r + 1, "club": clubs[*i].name, "pts": clubs[*i].pts, "diff": clubs[*i].diff, "match": clubs[*i].game, "outcome": clubs[*i].outcome, "pts0": clubs[*i].pts0, "diff0": clubs[*i].diff0})).collect::<Vec<_>>(),
        "matchs_comptes": counted,
        "carte": {"kind": "table", "title": title, "columns": ["#", "Club", "Pts", "Diff", "En cours"], "rows": table, "note": note, "live": {"rows": live, "points_col": 2, "match_col": 4}},
    });
    if !unmatched.is_empty() {
        out["non_apparies"] = json!(unmatched.into_iter().take(10).collect::<Vec<_>>());
        out["attention"] = "clubs introuvables dans le classement : renvoie leurs noms tels qu'écrits dans standings".into();
    }
    if !unreadable.is_empty() {
        out["scores_illisibles"] = json!(unreadable.into_iter().take(5).collect::<Vec<_>>());
    }
    Ok(Provisional { json: out })
}
