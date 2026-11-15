use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::model::Status;
use super::score::rank;
use super::state::Pronos;

const SEASON_SHOWN: usize = 20;

impl Pronos {
    pub fn payload(&mut self, now: f64) -> Option<Value> {
        let cancelled = self.current.is_none() && self.cancelled.is_some();
        let current = self.current.clone().or_else(|| self.cancelled.clone())?;
        self.calibration.for_match(Some(&current.id));
        let mut distribution: BTreeMap<&str, u32> = BTreeMap::new();
        current
            .votes
            .values()
            .for_each(|v| *distribution.entry(v).or_default() += 1);
        let mut distribution: Vec<(&str, u32)> = distribution.into_iter().collect();
        distribution.sort_by_key(|(score, n)| (std::cmp::Reverse(*n), *score));
        let mut season: Vec<Value> = self.season.iter().map(|(id, s)| json!({"user_id": id, "pseudo": s.name, "points": s.points, "gagnes": s.won})).collect();
        season.sort_by(|a, b| {
            (
                std::cmp::Reverse(a["points"].as_u64()),
                a["pseudo"].as_str().map(str::to_lowercase),
            )
                .cmp(&(
                    std::cmp::Reverse(b["points"].as_u64()),
                    b["pseudo"].as_str().map(str::to_lowercase),
                ))
        });
        season.truncate(SEASON_SHOWN);
        let diagnostic = self.calibration.diagnostic(
            self.default_delay_s,
            now,
            self.direct_key.as_ref(),
            self.margin_s,
        );
        let status = match current.status {
            Status::Open => "ouvert",
            Status::Running => "en_cours",
            Status::Final => "final",
        };
        let mut out = json!({
            "id": current.id, "competition": current.competition, "home": current.home, "away": current.away,
            "home_abbr": current.home_abbr, "away_abbr": current.away_abbr, "home_color": current.home_color, "away_color": current.away_color,
            "statut": status, "chaine": current.channel, "score": current.score, "minute": current.minute, "auto": current.auto,
            "ferme_dans_s": if current.status == Status::Open { ((current.closes_at - now) as i64).max(0) } else { 0 },
            "votes": current.votes, "noms": current.names, "requis": current.required,
            "distribution": distribution.iter().map(|(score, n)| json!({"score": score, "n": n})).collect::<Vec<_>>(),
            "classement_match": if current.status != Status::Open && !cancelled { json!(rank(&current.votes, &current.names, &current.score)) } else { json!([]) },
            "resultats": if current.status == Status::Final { json!(current.results) } else { json!([]) },
            "saison": season,
            "diagnostic": {"anti_spoiler": diagnostic},
        });
        if cancelled {
            out["annule"] = json!(true);
        }
        Some(out)
    }
}
