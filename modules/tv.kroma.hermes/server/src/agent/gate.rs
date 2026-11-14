use std::collections::BTreeSet;

use serde_json::Value;

use crate::votewords::parse_vote;

const KEYWORDS: &[&str] = &[
    "hermes", "hermès", "mets", "met ", "mettre", "metter", "zappe", "zapp", "change", "changer", "passe", "affiche", "affich", "chaine", "chaîne", "chaines", "chaînes", "match",
    "foot", "regarder", "regarde", "voir", "diffuse", "programme", "dessin", "film", "série", "serie", "bug", "saccade", "rame", "lag", "figé", "fige", "freeze", "plante",
    "planté", "cassé", "casse", "coupe", "coupé", "coupure", "gèle", "gele", "gelé", "son ", "image", "écran", "ecran", "combien", "remets", "remet", "scan", "teste", "tester",
    "secours", "fournisseur", "source", "flux", "noir", "cherche", "trouve", "compare", "vérifie", "verifie", "classement", "score", "résultat", "resultat", "quel ", "quelle",
    "qui joue", "qui gagne",
];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Signals {
    pub mentions_agent: bool,
    pub question: bool,
    pub keyword: bool,
}

impl Signals {
    pub fn of(text: &str, names: &[String], event: Option<&Value>) -> Self {
        let lowered = text.to_lowercase();
        let flag = |key: &str| event.and_then(|e| e.get(key)).and_then(Value::as_bool).unwrap_or(false);
        Self {
            mentions_agent: flag("mentionsAgent") || names.iter().any(|n| !n.trim().is_empty() && lowered.contains(&n.trim().to_lowercase())),
            question: flag("isQuestion") || text.trim().ends_with('?'),
            keyword: flag("keywordHit") || KEYWORDS.iter().any(|k| lowered.contains(k)),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Room {
    pub vote_required: Option<BTreeSet<String>>,
    pub poll_open: bool,
    pub spoke_recently: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Engage,
    Skip(&'static str),
}

/// Whether the assistant answers a message. A typed answer from someone a
/// vote waits for belongs to the vote, which the room counts itself. An open
/// poll lets every message through, because answers come without a keyword.
/// Otherwise: at least two characters, and either the assistant spoke within
/// the engage window, or a question mark, or a keyword.
pub fn decide(text: &str, signals: &Signals, user_id: &str, room: &Room) -> Verdict {
    if room.vote_required.as_ref().is_some_and(|r| r.contains(user_id)) && parse_vote(text).is_some() {
        return Verdict::Skip("vote_answer");
    }
    if room.poll_open {
        return Verdict::Engage;
    }
    if text.trim().chars().count() < 2 {
        return Verdict::Skip("too_short");
    }
    if room.spoke_recently || signals.mentions_agent || signals.question || signals.keyword {
        Verdict::Engage
    } else {
        Verdict::Skip("not_for_agent")
    }
}
