use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::event::{SportEvent, State};
use super::reading::{read_event, Certainty, Reading};

const END_MARGIN_S: f64 = 300.0;
const BLACK_MAX_AGE_S: f64 = 1800.0;
const OTHER_IMAGE_MAX_AGE_S: f64 = 900.0;
const WEB_KEEP_S: f64 = 86_400.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SeenChannel {
    pub channel_id: String,
    pub name: Option<String>,
    pub num: Option<i64>,
    pub state: String,
    pub event: Option<String>,
    pub certainty: Option<Certainty>,
    pub teams: Vec<String>,
    pub read_score: Option<String>,
    pub reading: Option<Reading>,
    pub at: f64,
    pub end: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct WebHint {
    pub query: Option<String>,
    pub at: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Memory {
    pub image: BTreeMap<String, SeenChannel>,
    pub web: BTreeMap<String, WebHint>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ScanRow {
    pub channel_id: String,
    pub name: Option<String>,
    pub num: Option<i64>,
    pub state: String,
    pub reading: Option<Reading>,
    pub from_guide: bool,
}

impl Memory {
    pub fn note_row(&mut self, row: &ScanRow, events: &[SportEvent], now: f64) -> bool {
        if row.state.is_empty() || row.channel_id.is_empty() {
            return false;
        }
        let previous = self.image.get(&row.channel_id).cloned().unwrap_or_default();
        let failed = matches!(row.state.as_str(), "erreur" | "non_teste")
            || (row.state == "antenne" && row.reading.is_none());
        if failed && previous.state == "direct" {
            return false;
        }
        let mut next = SeenChannel {
            channel_id: row.channel_id.clone(),
            name: row.name.clone(),
            num: row.num,
            state: if row.state == "ok" {
                "lecture".into()
            } else {
                row.state.clone()
            },
            at: now,
            ..SeenChannel::default()
        };
        if let Some(reading) = &row.reading {
            let found = read_event(reading, events);
            next.state = if reading.kind == "direct" {
                "direct".into()
            } else if reading.kind.is_empty() {
                "inconnu".into()
            } else {
                reading.kind.clone()
            };
            next.event = found.map(|(e, _)| e.id.clone());
            next.certainty = found.map(|(_, c)| {
                if row.from_guide {
                    Certainty::ToConfirm
                } else {
                    c
                }
            });
            next.teams = [&reading.team1, &reading.team2]
                .into_iter()
                .filter(|t| !t.is_empty())
                .cloned()
                .collect();
            next.read_score = Some(reading.score.clone()).filter(|s| !s.is_empty());
            next.reading = Some(reading.clone());
            next.end = found.map(|(e, _)| e.expected_end());
        }
        let unchanged = SeenChannel {
            at: previous.at,
            ..next.clone()
        } == previous;
        if unchanged {
            if let Some(entry) = self.image.get_mut(&row.channel_id) {
                entry.at = now;
            }
            return false;
        }
        self.image.insert(row.channel_id.clone(), next);
        true
    }

    pub fn expire(&mut self, events: &[SportEvent], now: f64) {
        let latest_start = events
            .iter()
            .filter(|e| e.start <= now)
            .map(|e| e.start)
            .fold(0.0, f64::max);
        self.image.retain(|_, seen| match seen.event.as_ref() {
            Some(event_id) => {
                let event = events.iter().find(|e| &e.id == event_id);
                let finished = match event {
                    Some(e) => e.state == State::Post,
                    None => now > seen.end.unwrap_or(0.0) + END_MARGIN_S,
                };
                !finished
            }
            None if matches!(seen.state.as_str(), "noir" | "erreur" | "non_teste") => {
                !(seen.at < latest_start || now - seen.at > BLACK_MAX_AGE_S)
            }
            None => now - seen.at <= OTHER_IMAGE_MAX_AGE_S,
        });
        let alive: Vec<&String> = events.iter().map(|e| &e.id).collect();
        self.web
            .retain(|id, hint| alive.contains(&id) || now - hint.at <= WEB_KEEP_S);
    }

    pub fn seen_channel(&self, event_id: &str) -> Option<&SeenChannel> {
        self.image
            .values()
            .filter(|s| s.event.as_deref() == Some(event_id))
            .max_by(|a, b| {
                let key = |s: &SeenChannel| (s.certainty == Some(Certainty::Seen), s.at);
                key(a)
                    .partial_cmp(&key(b))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }
}
