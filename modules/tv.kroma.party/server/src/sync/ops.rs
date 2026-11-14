use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use super::{Member,
    FineDone, FineReports, KickOutcome, Recal, Say, SyncGroup, CAP_S, FINE_FREEZE_MS,
    FINE_REPORT_TIMEOUT_MS, KICK_COOLDOWN_MS, KICK_WINDOW_MS, RECAL_GAP_MS, RECAL_MAX,
};
use crate::model::Person;

impl SyncGroup {
    pub fn fine_done(&mut self, who: &Person, done: FineDone, now: i64) -> Vec<Say> {
        let ms = done.ms.max(0);
        let seeks = done.seeks.max(0);
        if done.abandoned && !done.solo {
            self.log(now, json!({ "event": "synchro_fine_abandon", "userId": who.user_id, "name": who.name }));
            if let Some(r) = self.reports.as_mut() {
                r.got.insert(who.user_id.clone(), None);
                r.names.insert(who.user_id.clone(), who.name.clone());
            }
            return self.finish_reports_if_complete();
        }
        self.log(
            now,
            json!({ "event": "synchro_fine_resultat", "userId": who.user_id, "name": who.name,
                    "precisionMs": ms, "recalages": seeks, "atEntry": done.solo }),
        );
        if done.solo {
            let key = if ms <= 150 { "party.sync.fineSolo" } else { "party.sync.fineSoloMissed" };
            let secs = format!("{:.1}", ms as f64 / 1000.0);
            return vec![Say::new(key, &[("name", &who.name), ("ms", &ms.to_string()), ("seconds", &secs)], None)];
        }
        if let Some(r) = self.reports.as_mut() {
            r.got.insert(who.user_id.clone(), Some((ms, seeks)));
            r.names.insert(who.user_id.clone(), who.name.clone());
        }
        self.finish_reports_if_complete()
    }

    fn finish_reports_if_complete(&mut self) -> Vec<Say> {
        let complete = self.reports.as_ref().is_some_and(|r| {
            r.expected
                .iter()
                .all(|id| r.got.contains_key(id) || !self.members.contains_key(id))
        });
        if complete {
            self.finish_reports()
        } else {
            Vec::new()
        }
    }

    pub(crate) fn finish_reports(&mut self) -> Vec<Say> {
        let Some(r) = self.reports.take() else {
            return Vec::new();
        };
        if r.got.is_empty() {
            return Vec::new();
        }
        let items: Vec<Value> = r
            .got
            .iter()
            .map(|(id, res)| {
                let name = r.names.get(id).cloned().unwrap_or_default();
                match res {
                    Some((ms, seeks)) => json!({ "name": name, "ms": ms, "seeks": seeks, "abandoned": false }),
                    None => json!({ "name": name, "abandoned": true }),
                }
            })
            .collect();
        let mut say = Say::new("party.sync.fineDone", &[], None);
        say.params.insert("items".into(), Value::Array(items));
        vec![say]
    }

    pub(crate) fn reports_due(&self, now: i64) -> bool {
        self.reports.as_ref().is_some_and(|r| now >= r.deadline)
    }

    pub fn client_recalibration(&mut self, who: &Person, recal: Recal, now: i64) {
        if !self.members.contains_key(&who.user_id) {
            return;
        }
        let recent = self
            .recalibrations
            .iter()
            .rev()
            .find(|r| r["userId"] == who.user_id)
            .is_some_and(|r| now - r["ts"].as_i64().unwrap_or(0) < RECAL_GAP_MS);
        if recent {
            return;
        }
        let bound = |x: f64| {
            if x.is_finite() {
                (x.clamp(-120.0, 120.0) * 10.0).round() / 10.0
            } else {
                0.0
            }
        };
        let gap = bound(recal.gap);
        let mut ev = json!({
            "ts": now, "event": "recalage", "userId": who.user_id, "name": who.name,
            "gapS": gap, "direction": if gap < 0.0 { "catchUp" } else { "waitForGroup" },
            "targetS": bound(recal.target), "reason": if recal.arrival { "arrival" } else { "drift" },
        });
        if recal.blocked {
            ev["blocked"] = json!(true);
        }
        self.recalibrations.push_back(ev);
        if self.recalibrations.len() > RECAL_MAX {
            self.recalibrations.pop_front();
        }
    }

    fn disruptions(&self, user_id: &str, now: i64) -> usize {
        self.journal
            .iter()
            .filter(|e| {
                matches!(
                    e["event"].as_str(),
                    Some("decrochage" | "retard_en_rattrapage" | "plancher_releve")
                ) && e["userId"] == user_id
                    && now - e["ts"].as_i64().unwrap_or(0) < KICK_WINDOW_MS
            })
            .count()
    }

    pub fn kick(&mut self, user_id: &str, reachable: bool, reason: &str, now: i64) -> (KickOutcome, Vec<Say>) {
        let Some(member) = self.members.get(user_id) else {
            return (KickOutcome::Refused("notInParty"), Vec::new());
        };
        if !reachable {
            return (KickOutcome::Refused("disconnected"), Vec::new());
        }
        if self.last_kick.get(user_id).is_some_and(|t| now - t < KICK_COOLDOWN_MS) {
            return (KickOutcome::Refused("recentlyKicked"), Vec::new());
        }
        let name = member.name.clone();
        let count = self.disruptions(user_id, now);
        self.members.remove(user_id);
        self.last_kick.insert(user_id.to_string(), now);
        self.log(
            now,
            json!({ "event": "expulse", "userId": user_id, "name": name, "perturbations": count, "reason": reason }),
        );
        let key = if count > 0 { "party.sync.removedDisruptions" } else { "party.sync.removedRequested" };
        let say = Say::new(key, &[("name", &name), ("count", &count.to_string())], Some("join_party"));
        (
            KickOutcome::Kicked {
                disruptions: count,
                reason_stated: count > 0,
            },
            vec![say],
        )
    }

    pub fn go_live(&mut self, by: &str, now: i64) -> Result<Vec<Say>, &'static str> {
        self.freeze_until = 0;
        if self.members.is_empty() {
            return Err("noActiveParty");
        }
        self.target = 0.0;
        self.by = None;
        self.golive_at = now;
        self.log(now, json!({ "event": "retour_direct", "by": by }));
        Ok(vec![Say::new("party.sync.golive", &[], None)])
    }

    pub fn fine_sync(&mut self, by: &str, now: i64) -> Result<(), &'static str> {
        let measurable: Vec<(&String, &Member)> = self
            .members
            .iter()
            .filter(|(_, m)| self.fresh(m, now))
            .collect();
        if measurable.len() < 2 {
            return Err("needTwoMembers");
        }
        let rendezvous = self
            .members
            .values()
            .filter(|m| !m.paused && self.fresh(m, now))
            .fold(0.0_f64, |acc, m| acc.max(m.delay));
        if rendezvous > 0.0 {
            self.target = (rendezvous + 0.15).min(CAP_S);
        }
        self.freeze_until = now + FINE_FREEZE_MS;
        self.log(now, json!({ "event": "synchro_fine", "by": by }));
        self.reports = Some(FineReports {
            expected: self.members.keys().cloned().collect(),
            got: BTreeMap::new(),
            names: BTreeMap::new(),
            deadline: now + FINE_REPORT_TIMEOUT_MS,
        });
        Ok(())
    }

    pub fn info(&self, now: i64) -> Value {
        let members: Map<String, Value> = self
            .members
            .iter()
            .map(|(id, m)| {
                (
                    id.clone(),
                    json!({ "name": m.name, "delayS": m.delay, "paused": m.paused, "pinned": m.pinned,
                            "fresh": self.fresh(m, now) }),
                )
            })
            .collect();
        json!({ "active": !self.members.is_empty(), "members": members, "targetS": self.target,
                "by": self.by, "journal": self.journal(), "recalibrations": self.recalibrations() })
    }
}
