use std::collections::BTreeSet;

use crate::model::ScreenReading;

pub use parse::{parse_clock, parse_score};

mod parse;
mod teams;
#[cfg(test)]
mod tests;

pub const CONF_MIN: f64 = 0.6;
const CLOCK_TOL_S: f64 = 180.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Yes,
    Probable,
    Unknown,
    No,
}

impl Verdict {
    pub fn rank(self) -> i32 {
        match self {
            Verdict::Yes => 3,
            Verdict::Probable => 2,
            Verdict::Unknown => 1,
            Verdict::No => 0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Verdict::Yes => "oui",
            Verdict::Probable => "probable",
            Verdict::Unknown => "inconnu",
            Verdict::No => "non",
        }
    }
}

pub type Decision = (Verdict, String);

fn decision(verdict: Verdict, why: impl Into<String>) -> Decision {
    (verdict, why.into())
}

fn fold(text: &str) -> String {
    let lower: String = text.chars().map(fold_char).collect::<String>().to_lowercase();
    lower.split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ")
}

fn fold_char(c: char) -> char {
    match c {
        'à' | 'â' | 'ä' | 'á' | 'À' | 'Â' => 'a',
        'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' => 'e',
        'î' | 'ï' | 'í' | 'Î' => 'i',
        'ô' | 'ö' | 'ó' | 'Ô' => 'o',
        'ù' | 'û' | 'ü' | 'ú' | 'Ù' => 'u',
        'ç' | 'Ç' => 'c',
        'ñ' => 'n',
        c => c,
    }
}

pub fn canon_team(raw: &str) -> Option<String> {
    let folded = fold(raw);
    if folded.is_empty() {
        return None;
    }
    if let Some(name) = teams::alias(&folded) {
        return Some(name.to_string());
    }
    let kept: Vec<&str> = folded.split(' ').filter(|w| !teams::is_generic(w)).collect();
    let stripped = if kept.is_empty() { folded.clone() } else { kept.join(" ") };
    let name = teams::alias(&stripped).map_or(stripped, str::to_string);
    (!teams::is_ambiguous(&name)).then_some(name)
}

pub fn teams_of(reading: &ScreenReading) -> [Option<String>; 2] {
    [canon_team(&reading.team1).or_else(|| canon_team(&reading.tag1)), canon_team(&reading.team2).or_else(|| canon_team(&reading.tag2))]
}

pub fn has_teams(reading: &ScreenReading) -> bool {
    teams_of(reading).iter().any(Option::is_some)
}

fn team_set(reading: &ScreenReading) -> BTreeSet<String> {
    teams_of(reading).into_iter().flatten().collect()
}

fn direct(reading: &ScreenReading) -> bool {
    reading.kind == "direct"
}

pub fn decide(reference: &ScreenReading, candidate: &ScreenReading, dt_s: f64) -> Decision {
    if matches!(candidate.kind.as_str(), "noir" | "mire") {
        return decision(Verdict::No, "image noire ou mire");
    }
    if reference.confidence.min(candidate.confidence) < CONF_MIN {
        return decision(Verdict::Unknown, "lecture de l'image pas assez sûre");
    }
    let (tr, tc) = (teams_of(reference), teams_of(candidate));
    let (sr_, sc_) = (team_set(reference), team_set(candidate));
    if sr_.is_empty() || sc_.is_empty() {
        return decision(Verdict::Unknown, "équipes illisibles");
    }
    if sr_.is_disjoint(&sc_) {
        return decision(Verdict::No, "autres équipes");
    }
    let both_direct = direct(reference) && direct(candidate);
    let gap = match (parse_clock(&reference.clock), parse_clock(&candidate.clock)) {
        (Some(r), Some(c)) => Some(c as f64 - (r as f64 + dt_s)),
        _ => None,
    };
    let (sr, sc) = (parse_score(&reference.score), parse_score(&candidate.score));
    if sr_.len() == 2 && sr_ == sc_ {
        if !both_direct {
            return decision(Verdict::Probable, "mêmes équipes, mais pas deux directs (résumé, plateau ?)");
        }
        if let Some(g) = gap.filter(|g| g.abs() > CLOCK_TOL_S) {
            return decision(Verdict::Probable, format!("mêmes équipes, horloges écartées de {} min", g.abs() as i64 / 60));
        }
        if let (Some(sr), Some(mut sc)) = (sr, sc) {
            if tc[0] != tr[0] {
                sc = (sc.1, sc.0);
            }
            let diff = (sr.0 - sc.0).abs() + (sr.1 - sc.1).abs();
            if diff == 0 {
                return decision(Verdict::Yes, format!("mêmes équipes, même score {}-{}", sr.0, sr.1));
            }
            if dt_s > 0.0 && gap.is_some_and(|g| g.abs() <= CLOCK_TOL_S) && sc.0 >= sr.0 && sc.1 >= sr.1 {
                return decision(Verdict::Probable, "mêmes équipes, le score a avancé depuis la lecture du direct");
            }
            if diff == 1 {
                return decision(Verdict::Probable, "mêmes équipes, un but d'écart (décalage ?)");
            }
            return decision(Verdict::No, "mêmes équipes mais score différent");
        }
        if gap.is_some() {
            return decision(Verdict::Yes, "mêmes équipes, horloges concordantes");
        }
        return decision(Verdict::Probable, "mêmes équipes, score et horloge illisibles");
    }
    let same_score = matches!((sr, sc), (Some(a), Some(b)) if { let (mut x, mut y) = ([a.0, a.1], [b.0, b.1]); x.sort_unstable(); y.sort_unstable(); x == y });
    if both_direct && same_score && !gap.is_some_and(|g| g.abs() > CLOCK_TOL_S) {
        return decision(Verdict::Probable, "une seule équipe reconnue des deux côtés, même score");
    }
    decision(Verdict::Unknown, "une seule équipe en commun")
}

pub type Target = (Option<String>, Option<String>);

fn split_target(text: &str) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();
    let mut rest = text.trim();
    for sep in [" - ", " – ", " — ", " / ", " vs ", " vs. ", " contre ", " v "] {
        let lower = rest.to_lowercase();
        if let Some(at) = lower.find(sep) {
            parts = vec![rest[..at].trim().to_string(), rest[at + sep.len()..].trim().to_string()];
            rest = "";
            break;
        }
    }
    if rest.is_empty() { parts.into_iter().filter(|p| !p.is_empty()).collect() } else { vec![rest.to_string()] }
}

pub fn target_from_text(text: &str) -> Target {
    let parts = split_target(text);
    if parts.len() == 1 && !canon_team(&parts[0]).is_some_and(|c| teams::is_known(&c)) {
        let words: Vec<&str> = parts[0].split(|c: char| c.is_whitespace() || matches!(c, '-' | '–' | '—' | '/')).filter(|w| !w.is_empty()).collect();
        for cut in 1..words.len() {
            let (left, right) = (words[..cut].join(" "), words[cut..].join(" "));
            if canon_team(&left).is_some_and(|c| teams::is_known(&c)) && canon_team(&right).is_some_and(|c| teams::is_known(&c)) {
                return (Some(left), Some(right));
            }
        }
    }
    (parts.first().cloned(), parts.get(1).cloned())
}

pub fn match_target(target: &Target, candidate: &ScreenReading) -> Decision {
    if matches!(candidate.kind.as_str(), "noir" | "mire") {
        return decision(Verdict::No, "image noire ou mire");
    }
    if candidate.confidence < CONF_MIN {
        return decision(Verdict::Unknown, "lecture de l'image pas assez sûre");
    }
    let want: BTreeSet<String> = [target.0.as_deref(), target.1.as_deref()].into_iter().flatten().filter_map(canon_team).collect();
    let got = team_set(candidate);
    if want.is_empty() || got.is_empty() {
        return decision(Verdict::Unknown, "équipes illisibles");
    }
    if want.is_disjoint(&got) {
        return decision(Verdict::No, "autres équipes");
    }
    if want.len() == 2 && want == got {
        return if direct(candidate) {
            decision(Verdict::Yes, "les deux équipes recherchées, en direct")
        } else {
            decision(Verdict::Probable, "les deux équipes recherchées, mais pas un direct (résumé ?)")
        };
    }
    decision(Verdict::Probable, "une seule des équipes recherchées reconnue")
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GroupKey {
    Match(BTreeSet<String>, bool),
    Slot(String),
}

pub fn group_key(reading: Option<&ScreenReading>, slot: &str) -> GroupKey {
    let teams = reading.map(team_set).unwrap_or_default();
    match reading {
        Some(r) if teams.len() == 2 => GroupKey::Match(teams, direct(r)),
        _ => GroupKey::Slot(slot.to_string()),
    }
}

pub fn type_label(kind: &str) -> &str {
    match kind {
        "direct" => "direct",
        "resume" => "résumé / bande promo (pas un direct)",
        "plateau" => "plateau / studio",
        "mire" => "mire ou écran d'attente",
        "pub" => "publicité",
        "noir" => "écran noir",
        "autre" => "autre contenu",
        "inconnu" => "image non analysée",
        other => other,
    }
}

pub fn resume_fr(reading: Option<&ScreenReading>) -> String {
    let Some(r) = reading else { return "image non analysée".into() };
    let first = if r.team1.is_empty() { &r.tag1 } else { &r.team1 };
    let second = if r.team2.is_empty() { &r.tag2 } else { &r.team2 };
    let mut bits: Vec<String> = Vec::new();
    if !first.is_empty() && !second.is_empty() {
        bits.push(format!("{first} – {second}"));
    }
    for extra in [&r.score, &r.clock] {
        if !extra.is_empty() {
            bits.push(extra.clone());
        }
    }
    if bits.is_empty() && !r.description.is_empty() {
        bits.push(r.description.chars().take(60).collect());
    }
    let label = if r.kind == "direct" { "direct" } else { type_label(&r.kind) };
    if bits.is_empty() { label.to_string() } else { format!("{label} — {}", bits.join(", ")) }
}
