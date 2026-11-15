use serde::Serialize;

use super::model::{MatchPayload, Player};

pub fn revealed(t_image: Option<i64>, t_vu: i64, now_ms: f64, delay_s: f64) -> bool {
    match t_image {
        Some(t) => now_ms >= t as f64,
        None => now_ms >= t_vu as f64 + delay_s * 1000.0,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TeamLineup {
    #[serde(rename = "nom")]
    pub name: String,
    pub formation: String,
    pub onze_par_ligne: Vec<Vec<String>>,
    pub banc: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Substitution {
    #[serde(rename = "equipe")]
    pub team: String,
    pub minute: String,
    pub entre: String,
    pub sort: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LineupSummary {
    #[serde(rename = "match")]
    pub label: String,
    pub competition: String,
    #[serde(rename = "chaine")]
    pub channel: String,
    #[serde(rename = "statut")]
    pub state: String,
    #[serde(rename = "equipes")]
    pub teams: std::collections::BTreeMap<&'static str, TeamLineup>,
    pub changements_deja_faits: Vec<Substitution>,
    pub compos_publiees: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

fn label(player: &Player) -> String {
    if player.num != 0 {
        format!("{} {}", player.num, player.name).trim().to_string()
    } else {
        player.name.clone()
    }
}

pub fn lineup_summary(m: &MatchPayload, now_s: f64, delay_s: Option<f64>) -> LineupSummary {
    let delay = delay_s.unwrap_or(m.delay_s);
    let mut teams = std::collections::BTreeMap::new();
    let mut published = false;
    for side in ["dom", "ext"] {
        let starters: Vec<&Player> = m
            .players
            .iter()
            .filter(|p| p.side == side && p.starter)
            .collect();
        let bench: Vec<&Player> = m
            .players
            .iter()
            .filter(|p| p.side == side && !p.starter)
            .collect();
        published |= !starters.is_empty();
        let rows: Vec<Vec<String>> =
            if !starters.is_empty() && starters.iter().all(|p| p.row.is_some()) {
                let mut by_row: std::collections::BTreeMap<i64, Vec<&Player>> =
                    std::collections::BTreeMap::new();
                starters
                    .iter()
                    .for_each(|p| by_row.entry(p.row.unwrap_or(0)).or_default().push(p));
                by_row
                    .into_values()
                    .map(|mut row| {
                        row.sort_by(|a, b| a.x.unwrap_or(0.0).total_cmp(&b.x.unwrap_or(0.0)));
                        row.into_iter().map(label).collect()
                    })
                    .collect()
            } else if starters.is_empty() {
                Vec::new()
            } else {
                let mut sorted = starters.clone();
                sorted.sort_by_key(|p| p.place);
                vec![sorted.into_iter().map(label).collect()]
            };
        let info = m.teams.side(side);
        teams.insert(
            side_key(side),
            TeamLineup {
                name: info.name.clone(),
                formation: info.formation.clone(),
                onze_par_ligne: rows,
                banc: bench.into_iter().map(label).collect(),
            },
        );
    }
    let find = |id: &Option<String>| {
        id.as_ref()
            .and_then(|i| m.players.iter().find(|p| &p.id == i))
    };
    let changes = m
        .facts
        .iter()
        .filter(|f| f.kind == "remplacement" && revealed(f.t_image, f.t_vu, now_s * 1000.0, delay))
        .map(|f| Substitution {
            team: teams[side_key(&f.side)].name.clone(),
            minute: f.minute.clone(),
            entre: find(&f.came_on).map_or_else(|| "?".into(), label),
            sort: find(&f.went_off).map_or_else(|| "?".into(), label),
        })
        .collect();
    LineupSummary {
        label: format!("{} – {}", m.teams.dom.name, m.teams.ext.name),
        competition: m.competition.clone(),
        channel: m.channel.clone(),
        state: m.state.clone(),
        teams,
        changements_deja_faits: changes,
        compos_publiees: published,
        note: (!published).then(|| "compos pas encore publiées (ESPN les donne environ 1 h avant le coup d'envoi) : ne les invente pas".to_string()),
    }
}

fn side_key(side: &str) -> &'static str {
    if side == "dom" {
        "dom"
    } else {
        "ext"
    }
}
