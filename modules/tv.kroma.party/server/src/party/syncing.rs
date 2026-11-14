use serde_json::{json, Value};

use super::Party;
use crate::model::Person;
use crate::sync::{Beat, FineDone, KickOutcome, Recal, Say};

impl Party {
    pub(super) fn say_all(&self, says: Vec<Say>) {
        if says.is_empty() {
            return;
        }
        let mut st = self.lock();
        for s in says {
            self.system_message_with(&mut st, s.key, s.params, s.action);
        }
    }

    pub fn sync_beat(&self, who: &Person, beat: Beat) {
        let now = self.now();
        let says = self.lock().sync.beat(who, &beat, now);
        self.say_all(says);
    }

    pub fn sync_fine_done(&self, who: &Person, done: FineDone) {
        let now = self.now();
        let says = self.lock().sync.fine_done(who, done, now);
        self.say_all(says);
    }

    pub fn sync_recalibration(&self, who: &Person, recal: Recal) {
        let now = self.now();
        self.lock().sync.client_recalibration(who, recal, now);
    }

    pub fn sync_info(&self) -> Value {
        let now = self.now();
        self.lock().sync.info(now)
    }

    pub fn sync_go_live(&self, by: &str) -> Result<usize, &'static str> {
        let now = self.now();
        let mut st = self.lock();
        let says = st.sync.go_live(by, now)?;
        let count = st.sync.member_count();
        self.emit(&mut st, "party.sync.golive", json!({}));
        for s in says {
            self.system_message_with(&mut st, s.key, s.params, s.action);
        }
        Ok(count)
    }

    pub fn sync_fine_sync(&self, by: &str) -> Result<(), &'static str> {
        let now = self.now();
        let mut st = self.lock();
        st.sync.fine_sync(by, now)?;
        self.emit(&mut st, "party.sync.finesync", json!({}));
        Ok(())
    }

    pub fn kick(&self, user_id: &str, reason: &str) -> Result<usize, &'static str> {
        let reachable = self.fetch_presence().is_none_or(|v| v.iter().any(|p| p.user_id == user_id));
        let now = self.now();
        let mut st = self.lock();
        let (outcome, says) = st.sync.kick(user_id, reachable, reason, now);
        match outcome {
            KickOutcome::Refused(code) => Err(code),
            KickOutcome::Kicked { disruptions, reason_stated } => {
                for s in says {
                    self.system_message_with(&mut st, s.key, s.params, s.action);
                }
                let key = if reason_stated { "disruptions" } else { "requested" };
                self.emit_to(user_id, "party.sync.kicked", json!({ "reason": key, "count": disruptions }));
                Ok(disruptions)
            }
        }
    }

    pub fn time_now(&self) -> i64 {
        self.now()
    }
}
