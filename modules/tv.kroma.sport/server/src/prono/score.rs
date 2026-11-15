use std::collections::BTreeMap;
use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

pub const SCORE_MAX: u32 = 15;

static SCORE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d{1,2})-(\d{1,2})$").unwrap());

pub fn parse_score(text: &str) -> Option<(u32, u32)> {
    let m = SCORE.captures(text.trim())?;
    let (h, a): (u32, u32) = (m[1].parse().ok()?, m[2].parse().ok()?);
    (h <= SCORE_MAX && a <= SCORE_MAX).then_some((h, a))
}

fn outcome(score: (u32, u32)) -> std::cmp::Ordering {
    score.0.cmp(&score.1)
}

pub fn points(guess: &str, final_score: &str) -> u32 {
    let (Some(p), Some(f)) = (parse_score(guess), parse_score(final_score)) else {
        return 0;
    };
    if p == f {
        3
    } else {
        u32::from(outcome(p) == outcome(f))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Ranked {
    pub user_id: String,
    pub pseudo: String,
    pub prono: String,
    pub distance: u32,
    pub texte: String,
}

pub fn rank(
    votes: &BTreeMap<String, String>,
    names: &BTreeMap<String, String>,
    score: &str,
) -> Vec<Ranked> {
    let Some(current) = parse_score(score) else {
        return Vec::new();
    };
    let mut rows: Vec<(Ranked, bool)> = votes
        .iter()
        .filter_map(|(user, guess)| {
            let g = parse_score(guess)?;
            let pseudo = names.get(user).cloned().unwrap_or_else(|| user.clone());
            Some((
                Ranked {
                    user_id: user.clone(),
                    pseudo,
                    prono: guess.clone(),
                    distance: g.0.abs_diff(current.0) + g.1.abs_diff(current.1),
                    texte: String::new(),
                },
                outcome(g) == outcome(current),
            ))
        })
        .collect();
    rows.sort_by_key(|(r, ok)| (r.distance, !ok, r.pseudo.to_lowercase()));
    let Some(lead) = rows.first().map(|(r, ok)| (r.distance, *ok)) else {
        return Vec::new();
    };
    rows.into_iter()
        .map(|(mut r, ok)| {
            r.texte = if (r.distance, ok) == lead {
                "en tête".into()
            } else {
                format!(
                    "à {} but{}",
                    r.distance,
                    if r.distance > 1 { "s" } else { "" }
                )
            };
            r
        })
        .collect()
}

pub fn clock_seconds(text: &str) -> Option<i64> {
    static CLOCK: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(\d{1,3})\s*[:'’]\s*(\d{2})").unwrap());
    let m = CLOCK.captures(text)?;
    let seconds: i64 = m[2].parse().ok()?;
    (seconds < 60)
        .then(|| {
            m[1].parse::<i64>()
                .ok()
                .map(|minutes| minutes * 60 + seconds)
        })
        .flatten()
}
