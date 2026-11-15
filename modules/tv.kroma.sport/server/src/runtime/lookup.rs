use serde_json::{json, Value};

use super::Sport;
use crate::detect::{flat_matches, slug_competition, slug_espn, FlatMatch};
use crate::espn::{scoreboard, summary};
use crate::iso::day_key;
use crate::live_match::{extract, lineup_summary, Carry, Context};
use crate::match_facts::{facts, list_line, search};
use crate::standings::competition_code;

const LIST_MAX: usize = 25;
const FOUND_MAX: usize = 3;

impl Sport {
    fn board_for(&self, code: Option<&str>, now: f64) -> Option<Vec<FlatMatch>> {
        let code = code.unwrap_or("all");
        let mut lists = Vec::new();
        for day in [
            day_key(now - 24.0 * 3600.0),
            day_key(now),
            day_key(now + 24.0 * 3600.0),
        ] {
            let payload = self.espn.get(&scoreboard(code, &day)).ok()?;
            lists.push(flat_matches(&payload, code));
        }
        Some(crate::detect::merge(&lists))
    }

    pub(crate) fn assistant_delay(&self, now: f64) -> f64 {
        let default = self.settings().spoiler_delay_s;
        self.lock().pronos.delay(now).0.max(default)
    }

    /// Structured facts of a match (score, minute, scorers, cards) from ESPN, shifted so nothing precedes the picture.
    pub fn tool_match_facts(&self, teams: &str, competition: &str) -> Value {
        let now = self.now();
        let competition = competition.trim();
        let code = (!competition.is_empty())
            .then(|| competition_code(competition))
            .flatten();
        self.trace(
            "audit",
            "match_facts",
            json!({"competition": competition, "equipes": teams}),
        );
        let Some(mut matches) = self.board_for(code, now) else {
            return json!({"error": "ESPN injoignable : réessaie plus tard, ne devine pas le résultat"});
        };
        if let (Some(slug), None) = (
            slug_competition(competition).filter(|_| !competition.is_empty()),
            code,
        ) {
            let narrowed: Vec<FlatMatch> = matches
                .iter()
                .filter(|m| slug_espn(&m.competition) == Some(slug))
                .cloned()
                .collect();
            if !narrowed.is_empty() {
                matches = narrowed;
            }
        }
        let found: Vec<&FlatMatch> = if teams.is_empty() {
            Vec::new()
        } else {
            search(&matches, &json!(teams))
        };
        if found.is_empty() || found.len() > FOUND_MAX {
            let mut out = json!({"matchs_du_jour": matches.iter().map(list_line).take(LIST_MAX).collect::<Vec<_>>()});
            if !teams.is_empty() {
                out["attention"] = json!(if found.is_empty() {
                    "aucun match trouvé pour ces équipes aujourd'hui (ESPN)"
                } else {
                    "trop de matchs correspondent : précise les équipes"
                });
            }
            return out;
        }
        let delay = self.assistant_delay(now);
        let results: Vec<Value> = found
            .iter()
            .map(|m| {
                match self
                    .espn
                    .get(&summary("all", m.id.trim_start_matches("espn-")))
                {
                    Ok(s) => facts(&s, now, delay),
                    Err(e) => {
                        self.trace(
                            "espn",
                            "match_facts_source",
                            json!({"erreur": e.to_string()}),
                        );
                        json!({"match": m.label(), "error": "détails ESPN indisponibles"})
                    }
                }
            })
            .collect();
        json!({"matchs": results})
    }

    /// Real line-ups (ESPN, all competitions): the live match by default, else the named teams; substitutions only once revealed.
    pub fn tool_compos_match(&self, teams: &str) -> Value {
        let now = self.now();
        self.trace("audit", "compos_match", json!({"equipes": teams}));
        let teams = teams.trim();
        let (match_id, competition, followed) = if teams.is_empty() {
            let st = self.lock();
            match st.detect.matched.as_ref() {
                Some(m) => (m.found.id.clone(), m.found.competition.clone(), true),
                None => {
                    return json!({"error": "je ne reconnais pas de match sur la chaîne du direct", "conseil": "demande quelles équipes, puis rappelle compos_match(equipes=…)"})
                }
            }
        } else {
            let Some(matches) = self.board_for(None, now) else {
                return json!({"error": "ESPN injoignable : réessaie dans un instant, n'invente pas de compo"});
            };
            let mut found: Vec<&FlatMatch> = search(&matches, &json!(teams));
            if found.is_empty() {
                return json!({"error": format!("aucun match trouvé pour « {} » d'hier à demain (toutes compétitions ESPN) : demande de préciser, n'invente rien", teams.chars().take(60).collect::<String>())});
            }
            found.sort_by_key(|m| (rank(&m.state), (m.start.unwrap_or(now) - now).abs() as i64));
            let followed = self.lock().follow.target.as_deref() == Some(found[0].id.as_str());
            (found[0].id.clone(), found[0].competition.clone(), followed)
        };
        let delay = self.assistant_delay(now);
        if followed {
            if let Some(payload) = self
                .lock()
                .follow
                .payload
                .clone()
                .filter(|p| p.id == match_id)
            {
                return serde_json::to_value(lineup_summary(&payload, now, Some(delay)))
                    .unwrap_or(Value::Null);
            }
        }
        let fetched = match self
            .espn
            .get(&summary("all", match_id.trim_start_matches("espn-")))
        {
            Ok(s) => s,
            Err(e) => {
                self.trace(
                    "espn",
                    "compos_match_source",
                    json!({"erreur": e.to_string()}),
                );
                return json!({"error": "ESPN injoignable pour ce match : réessaie, n'invente pas de compo"});
            }
        };
        let mut seen = std::collections::BTreeMap::new();
        let payload = extract(
            &fetched,
            &mut Carry {
                seen: &mut seen,
                period_k: &Default::default(),
                possession: &[],
            },
            &Context {
                delay_s: delay,
                now_s: 0.0,
                channel: "",
                competition: &competition,
            },
        );
        serde_json::to_value(lineup_summary(&payload, now, Some(delay))).unwrap_or(Value::Null)
    }
}

fn rank(state: &str) -> u8 {
    match state {
        "in" => 0,
        "pre" => 1,
        "post" => 2,
        _ => 3,
    }
}
