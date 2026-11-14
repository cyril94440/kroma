use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use super::Party;

const SYNC_EVERY_MS: i64 = 2000;
const PRESENCE_EVERY_MS: i64 = 5000;
const STATUS_EVERY_MS: i64 = 15_000;
const PRUNE_EVERY_MS: i64 = 3_600_000;
const ACTIVITY_EXPIRES_MS: i64 = 120_000;
const UNSTABLE_MIN_MS: i64 = 120_000;
const ALERT_COOLDOWN_MS: i64 = 900_000;

fn due(last: &mut i64, now: i64, every: i64) -> bool {
    if now - *last >= every {
        *last = now;
        true
    } else {
        false
    }
}

impl Party {
    pub fn start(self: &Arc<Self>) {
        if self.started.swap(true, Ordering::SeqCst) {
            return;
        }
        let party = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(500)).await;
                let worker = party.clone();
                let _ = kroma_module_sdk::host::run_blocking(move || worker.tick()).await;
            }
        });
    }

    pub fn stop(&self) {
        self.started.store(false, Ordering::SeqCst);
    }

    pub fn tick(&self) {
        let now = self.now();
        self.expire_vote();
        self.expire_poll();
        let (sync_due, presence_due, status_due, prune_due) = {
            let mut st = self.lock();
            st.last_tick = now;
            if st.activity.as_ref().is_some_and(|(_, at)| now - at > ACTIVITY_EXPIRES_MS) {
                st.activity = None;
                self.emit(&mut st, "party.activity", json!({ "label": serde_json::Value::Null }));
            }
            (
                due(&mut st.last_sync_tick, now, SYNC_EVERY_MS),
                due(&mut st.last_presence_poll, now, PRESENCE_EVERY_MS),
                due(&mut st.last_status_poll, now, STATUS_EVERY_MS),
                due(&mut st.last_prune, now, PRUNE_EVERY_MS),
            )
        };
        if sync_due {
            let mut st = self.lock();
            let out = st.sync.tick(now);
            if let Some(state) = out.state {
                self.emit(&mut st, "party.sync", json!({ "state": state }));
            }
            for s in out.says {
                self.system_message_with(&mut st, s.key, s.params, s.action);
            }
        }
        if presence_due {
            self.refresh_presence();
            self.reevaluate_vote();
        }
        if status_due {
            self.check_instability();
        }
        if prune_due {
            if let Err(e) = self.archive() {
                tracing::error!(error = %e, "chat archive failed");
            }
        }
    }

    fn check_instability(&self) {
        let now = self.now();
        if !self.host.config_bool("instabilityAlerts", true) {
            self.lock().alert.unstable_since = None;
            return;
        }
        let Ok(status) = self.live.status() else {
            return;
        };
        let unstable = status.health.stutter || status.health.black || status.health.frozen || status.state == "error";
        let mut st = self.lock();
        if !unstable {
            st.alert.unstable_since = None;
            return;
        }
        let since = *st.alert.unstable_since.get_or_insert(now);
        let duration = now - since;
        let watched = st.present.as_ref().is_some_and(|p| !p.is_empty());
        if duration >= UNSTABLE_MIN_MS && now - st.alert.last_alert >= ALERT_COOLDOWN_MS && watched {
            st.alert.last_alert = now;
            let name = status.channel.map(|c| c.name).unwrap_or_default();
            let minutes = (duration / 60_000).to_string();
            self.system_message(&mut st, "party.alert.unstable", &[("channel", &name), ("minutes", &minutes)], None);
            self.trace("proactive_alert", json!({ "channel": name, "minutes": minutes }));
        }
    }
}
