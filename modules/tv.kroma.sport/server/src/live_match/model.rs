use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TeamInfo {
    #[serde(rename = "nom")]
    pub name: String,
    pub abbr: String,
    #[serde(rename = "couleur")]
    pub color: String,
    pub formation: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Teams {
    pub dom: TeamInfo,
    pub ext: TeamInfo,
}

impl Teams {
    pub fn side(&self, side: &str) -> &TeamInfo {
        if side == "dom" {
            &self.dom
        } else {
            &self.ext
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Player {
    pub id: String,
    #[serde(rename = "equipe")]
    pub side: String,
    pub num: i64,
    #[serde(rename = "nom")]
    pub name: String,
    #[serde(rename = "poste")]
    pub position: String,
    pub place: i64,
    #[serde(rename = "titulaire")]
    pub starter: bool,
    #[serde(rename = "ligne", skip_serializing_if = "Option::is_none")]
    pub row: Option<i64>,
    #[serde(rename = "lignes", skip_serializing_if = "Option::is_none")]
    pub rows: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Period {
    #[serde(rename = "periode")]
    pub number: i64,
    pub s0: i64,
    pub t_image: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Fact {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(rename = "equipe")]
    pub side: String,
    pub s: i64,
    #[serde(rename = "periode")]
    pub period: i64,
    pub minute: String,
    #[serde(rename = "joueur")]
    pub player: Option<String>,
    #[serde(rename = "passeur")]
    pub assist: Option<String>,
    #[serde(rename = "entre", skip_serializing_if = "Option::is_none")]
    pub came_on: Option<String>,
    #[serde(rename = "sort", skip_serializing_if = "Option::is_none")]
    pub went_off: Option<String>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub t_image: Option<i64>,
    pub t_vu: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Possession {
    pub t_image: i64,
    pub t_vu: i64,
    pub dom: f64,
    pub ext: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MatchPayload {
    pub id: String,
    pub competition: String,
    #[serde(rename = "chaine")]
    pub channel: String,
    #[serde(rename = "statut")]
    pub state: String,
    #[serde(rename = "equipes")]
    pub teams: Teams,
    #[serde(rename = "joueurs")]
    pub players: Vec<Player>,
    #[serde(rename = "periodes")]
    pub periods: Vec<Period>,
    #[serde(rename = "evenements")]
    pub facts: Vec<Fact>,
    #[serde(rename = "possession")]
    pub possession: Vec<Possession>,
    #[serde(rename = "delai_s")]
    pub delay_s: f64,
    #[serde(rename = "maj")]
    pub updated_ms: i64,
}
