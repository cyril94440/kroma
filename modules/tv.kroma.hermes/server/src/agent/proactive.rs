use serde_json::{json, Value};

use super::Agent;
use crate::strings;
use crate::webcache::normalise;

const OFFAIR_COOLDOWN_MS: i64 = 900_000;
const SUGGESTED: usize = 3;

const THEME_STOP: &[&str] = &["fr", "vip", "raw", "hd", "fhd", "uhd", "4k", "sd", "hevc", "canal", "canalplus", "tv", "channel", "the", "now", "prime", "live"];

fn themes(name: &str) -> Vec<String> {
    normalise(name).split(' ').filter(|w| !THEME_STOP.contains(w) && w.len() > 2 && !w.chars().all(|c| c.is_ascii_digit())).take(2).map(str::to_string).collect()
}

impl Agent {
    fn alternatives(&self, id: &str, name: &str, now: i64) -> Vec<String> {
        let mut found: Vec<(String, String)> = Vec::new();
        for theme in themes(name) {
            for p in self.deps.control.epg(Some(&theme), Some(now), Some(now + 1), 40).unwrap_or_default() {
                let live = p.start <= now && now < p.end;
                if live && p.channel_id != id && !found.iter().any(|(c, _)| *c == p.channel_name) {
                    found.push((p.channel_name, p.title));
                }
            }
        }
        found.into_iter().take(SUGGESTED).map(|(channel, title)| format!("{channel} ({title})")).collect()
    }

    pub(super) fn on_live_status(&self, payload: &Value) {
        let state = payload.get("state").and_then(Value::as_str).unwrap_or_default();
        self.deps.audit("sources", "live_status", json!({ "state": state, "channel": payload.pointer("/channel/name"), "health": payload.get("health") }));
        if state != "offAir" || !self.is_active() {
            return;
        }
        let watched = payload.get("viewers").and_then(Value::as_array).is_some_and(|v| !v.is_empty());
        let (id, name) = (payload.pointer("/channel/id").and_then(Value::as_str).unwrap_or_default(), payload.pointer("/channel/name").and_then(Value::as_str).unwrap_or_default());
        let now = self.deps.now();
        {
            let mut live = self.live();
            if !watched || name.is_empty() || now - live.last_offair < OFFAIR_COOLDOWN_MS {
                return;
            }
            live.last_offair = now;
        }
        let alternatives = self.alternatives(id, name, now);
        self.deps.audit("decisions", "offair_alternatives", json!({ "channel": name, "alternatives": alternatives }));
        let language = self.deps.settings().language;
        let key = if alternatives.is_empty() { "hermes.offair.none" } else { "hermes.offair.alternatives" };
        let text = strings::text(&language, key, &[("channel", name), ("alternatives", &alternatives.join(", "))]);
        let _ = self.deps.room.say(&text, None, None);
    }
}
