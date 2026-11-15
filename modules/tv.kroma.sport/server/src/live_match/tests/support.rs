use std::collections::BTreeMap;

use serde_json::{json, Value};

use crate::live_match::{extract, Carry, Context, MatchPayload, Possession};

pub const NOW_S: f64 = 1_793_552_000.0;
pub const NOW_MS: i64 = 1_793_552_000_000;

pub fn paris_monaco() -> Value {
    serde_json::from_str(include_str!(
        "../../../fixtures/espn_summary_parisfc_monaco.json"
    ))
    .unwrap()
}

pub fn villarreal() -> Value {
    serde_json::from_str(include_str!(
        "../../../fixtures/espn_summary_villarreal_psg_live.json"
    ))
    .unwrap()
}

#[derive(Default)]
pub struct Run {
    pub seen: BTreeMap<String, i64>,
    pub period_k: BTreeMap<i64, i64>,
    pub possession: Vec<Possession>,
    pub now_s: Option<f64>,
}

impl Run {
    pub fn extract(&mut self, summary: &Value) -> MatchPayload {
        let ctx = Context {
            delay_s: 60.0,
            now_s: self.now_s.unwrap_or(NOW_S),
            channel: "FR: LIGUE1+ 5",
            competition: "Ligue 1",
        };
        extract(
            summary,
            &mut Carry {
                seen: &mut self.seen,
                period_k: &self.period_k,
                possession: &self.possession,
            },
            &ctx,
        )
    }
}

pub fn extracted(summary: &Value) -> MatchPayload {
    Run::default().extract(summary)
}

pub fn play_mut<'a>(summary: &'a mut Value, id: &str) -> &'a mut Value {
    summary["commentary"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .filter_map(|i| i.get_mut("play"))
        .find(|p| p["id"] == id)
        .unwrap()
}

pub fn formation_summary(formation: &str, positions: &[&str]) -> Value {
    let roster: Vec<Value> = positions
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let n = i + 1;
            json!({"athlete": {"id": n.to_string(), "lastName": format!("J{n}"), "displayName": format!("J{n} X")}, "starter": true, "formationPlace": n, "jersey": n.to_string(), "position": {"abbreviation": p}})
        })
        .collect();
    json!({
        "header": {"id": "9", "competitions": [{"status": {"type": {"state": "in"}}, "competitors": [
            {"homeAway": "home", "team": {"id": "1", "displayName": "A"}}, {"homeAway": "away", "team": {"id": "2", "displayName": "B"}}]}]},
        "rosters": [{"homeAway": "home", "formation": formation, "roster": roster}],
        "commentary": []
    })
}
