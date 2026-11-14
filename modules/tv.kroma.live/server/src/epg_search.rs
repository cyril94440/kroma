use crate::db::StoredProgramme;
use crate::norm;

const STOP_WORDS: &[&str] = &[
    "le", "la", "les", "l", "un", "une", "des", "de", "du", "d", "au", "aux", "a", "et", "ou", "en", "sur", "dans", "avec", "pour", "que", "qui", "ce", "cet", "cette", "ces", "ca", "il", "elle", "on", "y", "est", "sont", "moi", "nous", "soir", "matin", "midi", "aujourd", "hui", "demain", "maintenant", "match", "matchs", "programme", "programmes", "emission", "emissions", "mets", "mettre", "affiche", "afficher", "regarde", "regarder", "voir", "chaine", "chaines", "direct", "live", "tv", "television", "stp", "svp", "plait",
];

pub fn tokens(query: &str) -> Vec<String> {
    let folded = norm::fold(query);
    let raw: Vec<String> = folded.split(|c: char| !c.is_alphanumeric()).filter(|w| w.chars().count() > 1).map(str::to_string).collect();
    let kept: Vec<String> = raw.iter().filter(|w| !STOP_WORDS.contains(&w.as_str())).cloned().collect();
    if kept.is_empty() { raw } else { kept }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub programme: StoredProgramme,
    pub score: usize,
    pub live: bool,
}

pub fn rank(candidates: Vec<StoredProgramme>, query_tokens: &[String], now_s: i64, limit: usize) -> Vec<Hit> {
    let mut full = Vec::new();
    let mut partial = Vec::new();
    for programme in candidates {
        if programme.stop < now_s {
            continue;
        }
        let score = query_tokens.iter().filter(|t| programme.hay.contains(t.as_str())).count();
        if score == 0 {
            continue;
        }
        let hit = Hit { live: programme.start <= now_s && now_s < programme.stop, programme, score };
        if score == query_tokens.len() { full.push(hit) } else { partial.push(hit) }
    }
    let mut chosen = if full.is_empty() && query_tokens.len() >= 2 { partial } else { full };
    chosen.sort_by(|a, b| b.score.cmp(&a.score).then(b.live.cmp(&a.live)).then(a.programme.start.cmp(&b.programme.start)));
    chosen.truncate(limit);
    chosen
}

#[cfg(test)]
mod tests {
    use super::*;

    fn programme(epg: &str, start: i64, stop: i64, hay: &str) -> StoredProgramme {
        StoredProgramme { epg_id: epg.into(), start, stop, title: hay.into(), hay: hay.into(), description: String::new() }
    }

    #[test]
    fn a_request_reduces_to_what_discriminates() {
        assert_eq!(tokens("le match de l'Argentine"), ["argentine"]);
        assert_eq!(tokens("mets le match"), ["mets", "le", "match"].iter().filter(|w| w.len() > 1).map(|w| w.to_string()).collect::<Vec<_>>());
        assert_eq!(tokens("Égypte – PSG"), ["egypte", "psg"]);
    }

    #[test]
    fn every_token_wins_over_a_partial_match_and_live_wins_over_later() {
        let found = rank(
            vec![
                programme("later", 500, 600, "psg liverpool"),
                programme("live", 0, 200, "psg liverpool"),
                programme("half", 0, 200, "psg nantes"),
                programme("over", 0, 50, "psg liverpool"),
            ],
            &["psg".to_string(), "liverpool".to_string()],
            100,
            8,
        );

        assert_eq!(found.iter().map(|h| h.programme.epg_id.as_str()).collect::<Vec<_>>(), ["live", "later"]);
    }

    #[test]
    fn a_partial_match_is_offered_only_when_nothing_matches_fully_and_the_request_has_several_tokens() {
        let candidates = vec![programme("a", 0, 200, "psg nantes")];

        assert_eq!(rank(candidates.clone(), &["psg".into(), "liverpool".into()], 100, 8).len(), 1);
        assert_eq!(rank(candidates, &["liverpool".into()], 100, 8).len(), 0);
    }
}
