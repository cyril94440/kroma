use serde::{Deserialize, Serialize};

use super::canon::{canon_team, IMAGE_CONFIDENCE_MIN};
use super::event::{Sport, SportEvent, State};
use super::guide::team_words;
use crate::text::fold;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Reading {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(rename = "equipe_1")]
    pub team1: String,
    #[serde(rename = "equipe_2")]
    pub team2: String,
    #[serde(rename = "sigle_1")]
    pub sigil1: String,
    #[serde(rename = "sigle_2")]
    pub sigil2: String,
    pub score: String,
    pub competition: String,
    #[serde(rename = "confiance")]
    pub confidence: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Certainty {
    #[serde(rename = "vu")]
    Seen,
    #[serde(rename = "a_confirmer")]
    ToConfirm,
}

pub fn read_event<'a>(
    reading: &Reading,
    events: &'a [SportEvent],
) -> Option<(&'a SportEvent, Certainty)> {
    if reading.kind != "direct" || reading.confidence < IMAGE_CONFIDENCE_MIN {
        return None;
    }
    let read = [
        (&reading.team1, &reading.sigil1),
        (&reading.team2, &reading.sigil2),
    ];
    let mut best: Option<(&SportEvent, usize)> = None;
    for event in events
        .iter()
        .filter(|e| e.sport == Sport::Soccer && e.state == State::In)
    {
        let mut hits = 0;
        for side in 0..2 {
            let Some(line) = event.lines.get(side) else {
                continue;
            };
            let words = team_words(&line.name);
            let sigil = event
                .abbreviations
                .get(side)
                .map(|s| fold(s))
                .unwrap_or_default();
            let canon = canon_team(&line.name);
            let matched = read.iter().any(|(name, sig)| {
                let padded = format!(" {} ", fold(name));
                words.iter().any(|w| padded.contains(&format!(" {w} ")))
                    || (!sigil.is_empty() && sigil == fold(sig))
                    || canon
                        .as_ref()
                        .is_some_and(|c| canon_team(name).as_ref() == Some(c))
            });
            hits += usize::from(matched);
        }
        if hits > best.map_or(0, |(_, n)| n) {
            best = Some((event, hits));
        }
    }
    best.map(|(event, hits)| {
        (
            event,
            if hits == 2 {
                Certainty::Seen
            } else {
                Certainty::ToConfirm
            },
        )
    })
}

/// What a live programme title says about the teams ("Ligue 1 : Lens - Marseille"), as a reading of moderate confidence: the guide, not the picture, named them.
pub fn reading_from_title(title: &str) -> Option<Reading> {
    let tail = title.split_once(':').map_or(title, |(_, rest)| rest);
    let teams = crate::match_facts::requested_teams(&serde_json::json!(tail));
    let [first, second] = teams.as_slice() else {
        return None;
    };
    Some(Reading {
        kind: "direct".into(),
        team1: first.clone(),
        team2: second.clone(),
        confidence: 0.65,
        ..Reading::default()
    })
}
