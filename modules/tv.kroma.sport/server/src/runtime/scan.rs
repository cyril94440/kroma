use serde_json::json;

use super::state::MEMORY_KEY;
use super::Sport;
use crate::sport::{need_scan, ScanStatus};

const OPEN_GAP_S: f64 = 15.0;
const FORCE_GAP_S: f64 = 10.0;
const POLL_S: f64 = 3.0;
const FAMILY: &str = "ligue1+";
const BENCHMARK_MAX_IDS: usize = 8;

impl Sport {
    /// Opening the sport view (or its refresh button) is the one gesture that may start a scan; a timer never does. Returns false when the opening was rate-limited.
    pub fn open_view(&self, user_id: &str, force: bool) -> bool {
        let now = self.now();
        let mut st = self.lock();
        let (gap, at) = if force {
            (FORCE_GAP_S, st.scan.forced_at)
        } else {
            (OPEN_GAP_S, st.scan.opened_at)
        };
        if now - at < gap {
            return false;
        }
        if force {
            st.scan.forced_at = now;
        }
        st.scan.opened_at = now;
        st.events.force = true;
        st.guide.relink_all = force;
        let events = st.events.list.clone();
        let need = need_scan(&events, &st.memory, now, force, st.scan.last_scan, &[]);
        drop(st);
        self.trace(
            "audit",
            "sport_ouvrir",
            json!({"force": force, "besoin": need.reason, "multiplex": need.multiplex}),
        );
        if need.multiplex {
            self.start_scan(now, user_id);
        }
        true
    }

    fn start_scan(&self, now: f64, requested_by: &str) {
        let ids: Vec<String> = self
            .live
            .resolve(FAMILY, BENCHMARK_MAX_IDS as u32)
            .map(|found| found.into_iter().map(|c| c.id).collect())
            .unwrap_or_default();
        let accepted = !ids.is_empty()
            && self
                .live
                .benchmark(&ids, requested_by)
                .ok()
                .flatten()
                .is_some_and(|b| b.status == "started");
        let mut st = self.lock();
        st.scan.last_scan = now;
        st.scan.status = if accepted {
            ScanStatus {
                state: "en_cours".into(),
                ..ScanStatus::default()
            }
        } else {
            ScanStatus {
                state: "impossible".into(),
                reason: Some("Scan indisponible pour l'instant".into()),
                ..ScanStatus::default()
            }
        };
    }

    pub(crate) fn step_scan(&self, now: f64) -> bool {
        {
            let st = self.lock();
            if st.scan.status.state != "en_cours" || now - st.scan.poll_at < POLL_S {
                return false;
            }
        }
        self.lock().scan.poll_at = now;
        let report = self.live.scan_result();
        let mut st = self.lock();
        match report {
            Ok(Some(report)) => {
                let events = st.events.list.clone();
                let mut changed = false;
                for row in &report.rows {
                    changed |= st.memory.note_row(row, &events, now);
                }
                st.scan.status.done = report.rows.len() as u32;
                st.scan.status.total = report.total as u32;
                st.scan.status.captures = report.rows.len() as u32;
                if !report.running {
                    st.scan.status.state = "fait".into();
                    st.scan.status.fin_ts = Some((now * 1000.0) as i64);
                }
                if changed || st.scan.status.state == "fait" {
                    self.store.kv_put(MEMORY_KEY, &st.memory);
                }
            }
            Ok(None) | Err(_) => {
                st.scan.status = ScanStatus {
                    state: "impossible".into(),
                    reason: Some("Scan indisponible pour l'instant".into()),
                    ..ScanStatus::default()
                };
            }
        }
        true
    }
}
