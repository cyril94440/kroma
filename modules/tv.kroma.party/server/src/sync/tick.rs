use serde_json::json;

use super::{SyncGroup, SyncState, Say, BAND_S, CAP_S, PURGE_MS, STUCK_THRESHOLD, STUCK_WINDOW_MS};
use crate::model::Person;

pub struct Tick {
    pub state: Option<SyncState>,
    pub says: Vec<Say>,
}

impl SyncGroup {
    pub fn tick(&mut self, now: i64) -> Tick {
        let mut says = Vec::new();
        if self.reports_due(now) {
            says.extend(self.finish_reports());
        }
        let gone: Vec<(String, String)> = self
            .members
            .iter()
            .filter(|(_, m)| now - m.ts > PURGE_MS)
            .map(|(id, m)| (id.clone(), m.name.clone()))
            .collect();
        for (id, name) in gone {
            self.members.remove(&id);
            self.stuck_clear(&id);
            self.log(
                now,
                json!({ "event": "quitte", "userId": id, "name": name, "cause": "noHeartbeat45s" }),
            );
        }
        if self.members.is_empty() {
            let announce_empty = std::mem::take(&mut self.was_active);
            self.target = 0.0;
            self.by = None;
            let state = announce_empty.then(|| self.state(0.0, now));
            return Tick { state, says };
        }
        self.was_active = true;
        let mut max_d = 0.0_f64;
        let mut max_user: Option<String> = None;
        let mut min_d = f64::INFINITY;
        for (id, m) in &self.members {
            if m.paused || !self.fresh(m, now) {
                continue;
            }
            if m.delay > max_d {
                max_d = m.delay;
                max_user = Some(id.clone());
            }
            if m.delay < min_d {
                min_d = m.delay;
            }
        }
        let Some(max_user) = max_user else {
            return Tick { state: Some(self.state(max_d, now)), says };
        };
        let recent_resume = self.members.values().any(|m| {
            !m.paused && self.fresh(m, now) && m.paused_at > 0 && now - m.paused_at < 30_000
        });
        if self.target >= 1.0 && now >= self.freeze_until {
            let before = self.target;
            if self.target < min_d - BAND_S && !recent_resume {
                self.target = (min_d - BAND_S).max(max_d - 1.5);
            } else if self.target > max_d + BAND_S {
                self.target = max_d + BAND_S;
            }
            if (self.target - before).abs() > 1.0 {
                let cause = if self.target > before { "sourceHiccup" } else { "groupBackToLive" };
                self.log(
                    now,
                    json!({ "event": "groupe_decale", "from": round1(before), "to": round1(self.target), "cause": cause }),
                );
                self.by = None;
            }
        }
        if max_d > self.target + 2.5 {
            self.on_laggard(&max_user, max_d, now);
        } else if now >= self.freeze_until {
            self.drain(max_d, min_d, now);
        }
        Tick { state: Some(self.state(max_d, now)), says }
    }

    fn on_laggard(&mut self, user: &str, max_d: f64, now: i64) {
        let (paused_at, name) = self
            .members
            .get(user)
            .map(|m| (m.paused_at, m.name.clone()))
            .unwrap_or_default();
        let recent_pause = paused_at > 0 && now - paused_at < 30_000;
        let raised = max_d.min(CAP_S);
        if self.target < 1.0 {
            self.log(now, json!({ "event": "calage_initial", "userId": user, "name": name, "to": round1(raised) }));
            self.target = raised;
            self.by = Some(user.to_string());
        } else if recent_pause {
            if raised - self.target >= 0.5 {
                self.log(
                    now,
                    json!({ "event": "decrochage", "userId": user, "name": name, "cause": "resumeAfterPause",
                            "from": round1(self.target), "to": round1(raised) }),
                );
            }
            self.target = raised;
            self.by = Some(user.to_string());
        } else if raised - self.target >= 0.5 && self.is_stuck(user, now) {
            self.log(
                now,
                json!({ "event": "plancher_releve", "userId": user, "name": name, "cause": "noBufferToCatchUp",
                        "from": round1(self.target), "to": round1(raised) }),
            );
            self.target = raised;
            self.by = Some(user.to_string());
            self.stuck_clear(user);
        } else {
            let last = self.journal.iter().rev().find(|e| {
                e["event"] == "retard_en_rattrapage" && e["userId"] == user
            });
            let due = last.is_none_or(|e| now - e["ts"].as_i64().unwrap_or(0) > 60_000);
            if due {
                self.log(
                    now,
                    json!({ "event": "retard_en_rattrapage", "userId": user, "name": name, "cause": "bufferingOrSlowLink",
                            "lagS": round1(max_d), "targetS": round1(self.target) }),
                );
            }
        }
    }

    fn drain(&mut self, max_d: f64, min_d: f64, now: i64) {
        let floor = self
            .members
            .values()
            .filter(|m| !m.paused && self.fresh(m, now) && m.pinned)
            .fold(0.0_f64, |acc, m| acc.max(m.delay));
        let mut next = floor.min(CAP_S).max(self.target - 0.1);
        if next < max_d - 1.5 {
            next = self.target.min(max_d - 1.5);
        }
        if next < min_d - BAND_S {
            next = self.target.min(min_d - BAND_S);
        }
        self.target = next;
        if self.by.is_some() && (self.target <= floor + 0.3 || self.target >= max_d - 1.6) {
            self.log(now, json!({ "event": "groupe_resynchronise", "latencyS": round1(self.target) }));
            self.by = None;
        }
    }

    fn is_stuck(&mut self, user: &str, now: i64) -> bool {
        let blocked = self.members.get(user).is_some_and(|m| m.pinned || m.blocked);
        let seen = self.stuck.entry(user.to_string()).or_default();
        seen.retain(|t| now - t <= STUCK_WINDOW_MS);
        if blocked {
            seen.push(now);
        }
        seen.len() >= STUCK_THRESHOLD
    }

    fn stuck_clear(&mut self, user: &str) {
        self.stuck.remove(user);
    }

    fn state(&mut self, max_d: f64, now: i64) -> SyncState {
        let capped = max_d > CAP_S;
        if self.by.as_ref().is_some_and(|b| !self.members.contains_key(b)) {
            self.by = None;
        }
        SyncState {
            target: (self.target * 100.0).round() / 100.0,
            by: self
                .by
                .as_ref()
                .and_then(|id| self.members.get(id).map(|m| Person::new(id, &m.name))),
            capped,
            members: self.members.iter().map(|(id, m)| Person::new(id, &m.name)).collect(),
            active: self.members.values().filter(|m| self.fresh(m, now)).count(),
        }
    }
}

fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}


impl SyncGroup {
    pub fn peek(&self, now: i64) -> Option<SyncState> {
        if self.members.is_empty() {
            return None;
        }
        let max_d = self
            .members
            .values()
            .filter(|m| !m.paused && self.fresh(m, now))
            .fold(0.0_f64, |acc, m| acc.max(m.delay));
        Some(SyncState {
            target: (self.target * 100.0).round() / 100.0,
            by: self
                .by
                .as_ref()
                .and_then(|id| self.members.get(id).map(|m| Person::new(id, &m.name))),
            capped: max_d > CAP_S,
            members: self.members.iter().map(|(id, m)| Person::new(id, &m.name)).collect(),
            active: self.members.values().filter(|m| self.fresh(m, now)).count(),
        })
    }
}
