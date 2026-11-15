use regex::Regex;

use super::canon::GENERIC;
use super::event::{Sport, SportEvent};
use crate::text::fold;

const EXTRA_EMPTY: [&str; 11] = [
    "cd", "cf", "sv", "vfb", "tsg", "aif", "nk", "sk", "kaa", "real", "city",
];
const EXTRA_EMPTY_MORE: [&str; 3] = ["united", "town", "saint"];

const FRENCH_ALIASES: [(&str, &str); 16] = [
    ("sevilla", "seville"),
    ("barcelona", "barcelone"),
    ("bayern munich", "bayern"),
    ("napoli", "naples"),
    ("red star belgrade", "etoile rouge"),
    ("inter milan", "inter"),
    ("ac milan", "milan"),
    ("genoa", "genes"),
    ("mallorca", "majorque"),
    ("fiorentina", "fiorentina"),
    ("heart of midlothian", "hearts"),
    ("real sociedad", "sociedad"),
    ("paris saint germain", "psg"),
    ("psv eindhoven", "psv"),
    ("sporting cp", "sporting"),
    ("atletico madrid", "atletico"),
];

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GuideProgramme {
    pub channel_id: String,
    pub channel_name: String,
    pub title: String,
    pub description: String,
    pub start_s: f64,
    pub end_s: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProgrammeChoice {
    pub channel_id: String,
    pub channel_name: String,
    pub title: String,
}

fn is_empty_word(word: &str) -> bool {
    GENERIC.contains(&word) || EXTRA_EMPTY.contains(&word) || EXTRA_EMPTY_MORE.contains(&word)
}

fn french_alias(folded: &str) -> Option<&'static str> {
    FRENCH_ALIASES
        .iter()
        .find(|(from, _)| *from == folded)
        .map(|(_, to)| *to)
}

pub fn team_words(name: &str) -> Vec<String> {
    let folded = fold(name);
    let mut words: Vec<String> = folded
        .split(' ')
        .filter(|w| !is_empty_word(w) && w.len() >= 3)
        .map(str::to_string)
        .collect();
    if let Some(alias) = french_alias(&folded) {
        words.insert(0, alias.to_string());
    }
    if words.is_empty() {
        vec![folded]
    } else {
        words
    }
}

pub fn title_covers(title: &str, a: &str, b: &str) -> bool {
    let padded = format!(" {} ", fold(title));
    [a, b].iter().all(|team| {
        team_words(team)
            .iter()
            .any(|word| padded.contains(&format!(" {word} ")))
    })
}

pub fn guide_queries(event: &SportEvent) -> Vec<String> {
    let tournament = event.tournament.clone().unwrap_or_default();
    match (event.sport, event.team_names()) {
        (Sport::Soccer, Some((a, b))) => {
            let swap =
                |n: &str| french_alias(&fold(n)).map_or_else(|| n.to_string(), str::to_string);
            let (a, b) = (swap(a), swap(b));
            vec![format!("{a} {b}"), b, a]
        }
        (Sport::Golf, _) => vec!["golf pga tour".into(), tournament],
        (Sport::F1, _) => {
            let trimmed = Regex::new(r"(?i)grand prix")
                .unwrap()
                .replace_all(&tournament, "")
                .trim()
                .to_string();
            vec![format!("formule 1 {trimmed}"), tournament]
        }
        _ => vec![format!(
            "tennis {}",
            if tournament.is_empty() {
                &event.competition
            } else {
                &tournament
            }
        )],
    }
}

fn sport_words(sport: Sport) -> &'static [&'static str] {
    match sport {
        Sport::Tennis => &["tennis", "masters", "open", "atp", "wta"],
        Sport::F1 => &["formule", "f1", "grand prix", "gp"],
        Sport::Golf => &["golf", "pga", "championship"],
        Sport::Soccer => &[],
    }
}

fn fits(programme: &GuideProgramme, event: &SportEvent, now: f64) -> bool {
    let title = &programme.title;
    let folded = fold(title);
    if ["termine", "resume", "rediffusion", "replay"]
        .iter()
        .any(|p| folded.starts_with(p))
    {
        return false;
    }
    let teams = event.team_names();
    if folded.starts_with("a venir") && event.sport == Sport::Soccer {
        return teams.is_some_and(|(a, b)| {
            (programme.end_s - event.start).abs() <= 1800.0 && title_covers(title, a, b)
        });
    }
    let instant = if event.start <= now && event.sport != Sport::Soccer {
        now
    } else {
        event.start + 1200.0
    };
    if !(programme.start_s <= instant && instant <= programme.end_s) {
        return false;
    }
    match event.sport {
        Sport::Soccer => teams.is_some_and(|(a, b)| title_covers(title, a, b)),
        Sport::Golf => folded.contains("golf") && folded.contains("pga"),
        sport => {
            let tournament = fold(event.tournament.as_deref().unwrap_or_default());
            let words: Vec<&str> = tournament
                .split(' ')
                .filter(|w| w.len() >= 4 && !["grand", "prix", "championship"].contains(w))
                .collect();
            sport_words(sport).iter().any(|w| folded.contains(w))
                && (words.is_empty() || words.iter().any(|w| folded.contains(w)))
        }
    }
}

pub fn choose_programme(
    candidates: &[GuideProgramme],
    event: &SportEvent,
    now: f64,
    preference: impl Fn(&str) -> f64,
) -> Option<ProgrammeChoice> {
    let mut best: Option<(f64, &GuideProgramme)> = None;
    for programme in candidates.iter().filter(|p| fits(p, event, now)) {
        let score = preference(&programme.channel_name);
        if best.is_none_or(|(top, _)| score > top) {
            best = Some((score, programme));
        }
    }
    best.map(|(_, p)| ProgrammeChoice {
        channel_id: p.channel_id.clone(),
        channel_name: p.channel_name.clone(),
        title: p.title.clone(),
    })
}
