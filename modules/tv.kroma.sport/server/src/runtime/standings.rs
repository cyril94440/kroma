use serde_json::{json, Value};

use super::Sport;
use crate::espn::{live_scores, standings};
use crate::standings::{competition_code, from_espn, provisional, LiveGame, StandingRow};

const TTL_S: f64 = 120.0;

impl Sport {
    /// The provisional table of a competition. The model may hand its own rows only when the source is unreachable; the arithmetic is always the code's.
    pub fn live_standings(
        &self,
        competition: &str,
        rows: Option<Vec<StandingRow>>,
        games: Option<Vec<LiveGame>>,
        top: i64,
    ) -> Value {
        let name = if competition.trim().is_empty() {
            "Ligue 1"
        } else {
            competition
        };
        let code = competition_code(name);
        let (mut rows, mut games, mut source) = (
            rows.unwrap_or_default(),
            games.unwrap_or_default(),
            "modele",
        );
        if let Some(code) = code {
            match (
                self.espn.get(&standings(code)),
                self.espn.get(&live_scores(code)),
            ) {
                (Ok(table), Ok(scores)) => {
                    let (from_rows, from_games) = from_espn(&table, &scores);
                    if !from_rows.is_empty() {
                        (rows, games, source) = (from_rows, from_games, "espn");
                    }
                }
                (a, b) => self.trace(
                    "espn",
                    "live_standings_source",
                    json!({"erreur": a.err().or(b.err()).map(|e| e.to_string())}),
                ),
            }
        }
        if rows.is_empty() {
            return json!({"error": format!("classement de {name} indisponible (source automatique injoignable) : ne l'invente pas, dis-le")});
        }
        match provisional(&rows, &games, name, top) {
            Ok(result) => {
                let mut out = result.json;
                out["source"] = json!(source);
                self.trace("audit", "live_standings", json!({"clubs": out["lignes"].as_array().map(Vec::len), "matchs": out["matchs_comptes"], "non_apparies": out.get("non_apparies")}));
                out
            }
            Err(e) => json!({"error": e}),
        }
    }

    /// The cached provisional table for the page, or `None` while a background read is queued.
    pub fn standings_cached(&self, competition: &str, now: f64) -> Option<Value> {
        let code = competition_code(competition)?.to_string();
        let mut st = self.lock();
        match st.standings.cache.get(&code) {
            Some((at, value)) if now - at < TTL_S => Some(value.clone()),
            stale => {
                let value = stale.map(|(_, v)| v.clone());
                st.standings.pending.insert(competition.to_string());
                value
            }
        }
    }

    pub(crate) fn step_standings(&self, now: f64) -> bool {
        let Some(name) = self.lock().standings.pending.iter().next().cloned() else {
            return false;
        };
        self.lock().standings.pending.remove(&name);
        let value = self.live_standings(&name, None, None, 20);
        if let Some(code) = competition_code(&name) {
            if value.get("error").is_none() {
                self.lock()
                    .standings
                    .cache
                    .insert(code.to_string(), (now, value));
            }
        }
        true
    }
}
