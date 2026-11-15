use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::calibrage::Persisted;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    #[serde(rename = "ouvert")]
    Open,
    #[serde(rename = "en_cours")]
    Running,
    #[serde(rename = "final")]
    Final,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultRow {
    pub user_id: String,
    pub pseudo: String,
    pub prono: String,
    pub points: u32,
    pub exact: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Current {
    pub id: String,
    pub code: String,
    pub competition: String,
    pub home: String,
    pub away: String,
    pub home_abbr: String,
    pub away_abbr: String,
    pub home_color: String,
    pub away_color: String,
    pub status: Status,
    pub closes_at: f64,
    pub opened_mid_match: bool,
    pub kickoff: f64,
    pub channel: String,
    pub score: String,
    pub minute: String,
    pub auto: bool,
    pub votes: BTreeMap<String, String>,
    pub names: BTreeMap<String, String>,
    pub required: Vec<String>,
    pub results: Vec<ResultRow>,
    pub counted: bool,
    pub missed: u32,
    pub opened_at: f64,
    pub away_since: Option<f64>,
    pub ended_at: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Season {
    pub name: String,
    pub points: u32,
    pub won: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct File {
    pub seen: Vec<String>,
    pub season: BTreeMap<String, Season>,
    pub current: Option<Current>,
    pub calibration: Option<Persisted>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    Opened,
    Closed,
    Updated,
    Finished,
    Abandoned,
    Away,
}
