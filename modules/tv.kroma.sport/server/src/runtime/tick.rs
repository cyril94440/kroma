use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use super::Sport;

const TICK_MS: u64 = 500;

impl Sport {
    pub fn start(self: &Arc<Self>) {
        if self.started.swap(true, Ordering::SeqCst) {
            return;
        }
        let sport = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(TICK_MS)).await;
                if !sport.started.load(Ordering::SeqCst) {
                    continue;
                }
                let worker = sport.clone();
                let _ = kroma_module_sdk::host::run_blocking(move || worker.tick()).await;
            }
        });
    }

    pub fn stop(&self) {
        self.started.store(false, Ordering::SeqCst);
    }

    /// One tick does at most one slow thing (a single network read, or a handful of local calls to the live module), so the single-instance component is never held for long.
    pub fn tick(&self) {
        if !self.settings().enabled {
            return;
        }
        let now = self.now();
        self.step_status(now);
        let _ = self.step_scan(now)
            || self.step_detect(now)
            || self.step_follow(now)
            || self.step_prono(now)
            || self.step_clips(now)
            || self.step_sweep(now)
            || self.step_guide(now)
            || self.step_standings(now);
        self.publish_changes(now);
    }
}
