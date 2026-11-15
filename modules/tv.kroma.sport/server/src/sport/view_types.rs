use serde::{Deserialize, Serialize};

use super::event::Line;
use super::reading::Certainty;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ChannelLink {
    pub name: String,
    #[serde(rename = "ref")]
    pub channel_id: Option<String>,
    pub source: String,
    pub at: f64,
    pub foreign: bool,
}

#[derive(Debug, Clone, Default)]
pub struct OnScreen {
    pub channel_id: Option<String>,
    pub espn: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScanStatus {
    #[serde(rename = "etat")]
    pub state: String,
    #[serde(rename = "fait")]
    pub done: u32,
    pub total: u32,
    pub captures: u32,
    pub fin_ts: Option<i64>,
    #[serde(rename = "raison")]
    pub reason: Option<String>,
}

impl Default for ScanStatus {
    fn default() -> Self {
        Self {
            state: "inactif".into(),
            done: 0,
            total: 0,
            captures: 0,
            fin_ts: None,
            reason: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChannelView {
    #[serde(rename = "nom")]
    pub name: String,
    #[serde(rename = "ref")]
    pub channel_id: Option<String>,
    pub source: &'static str,
    #[serde(rename = "certitude")]
    pub certainty: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vu_ts: Option<i64>,
    #[serde(rename = "etrangere", skip_serializing_if = "std::ops::Not::not")]
    pub foreign: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EventView {
    pub id: String,
    pub sport: &'static str,
    #[serde(rename = "compet")]
    pub competition: String,
    #[serde(rename = "statut")]
    pub state: &'static str,
    #[serde(rename = "debut")]
    pub start_ms: i64,
    #[serde(rename = "temps")]
    pub clock: String,
    #[serde(rename = "lignes")]
    pub lines: Vec<Line>,
    #[serde(rename = "chaine")]
    pub channel: Option<ChannelView>,
    #[serde(rename = "a_lecran")]
    pub on_screen: bool,
    #[serde(rename = "masquer_score")]
    pub hide_score: bool,
    pub ligue1: bool,
    #[serde(rename = "pop")]
    pub popularity: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MultiplexChannel {
    pub num: Option<i64>,
    #[serde(rename = "nom")]
    pub name: Option<String>,
    #[serde(rename = "ref")]
    pub channel_id: String,
    #[serde(rename = "etat")]
    pub state: String,
    pub event: Option<String>,
    #[serde(rename = "certitude")]
    pub certainty: Option<Certainty>,
    #[serde(rename = "equipes")]
    pub teams: Vec<String>,
    pub score: String,
    #[serde(rename = "temps")]
    pub clock: String,
    pub vu_ts: i64,
    #[serde(rename = "a_lecran")]
    pub on_screen: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Ligue1View {
    #[serde(rename = "direct")]
    pub live: usize,
    #[serde(rename = "prochain")]
    pub next_ms: Option<i64>,
    #[serde(rename = "canaux")]
    pub channels: Vec<MultiplexChannel>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SportView {
    #[serde(rename = "maj")]
    pub updated_ms: i64,
    #[serde(rename = "direct")]
    pub live: usize,
    #[serde(rename = "prochain")]
    pub next_ms: Option<i64>,
    #[serde(rename = "evenements")]
    pub events: Vec<EventView>,
    pub ligue1: Ligue1View,
    pub scan: ScanStatus,
}
