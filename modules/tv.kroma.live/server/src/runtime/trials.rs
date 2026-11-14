use std::sync::Arc;
use std::time::{Duration, Instant};

use kroma_module_sdk::host::Event;
use kroma_module_sdk::host::process::Command;
use serde_json::json;

use super::connections::Use;
use super::media;
use super::process::stop_child;
use super::snapshot::TOPIC_FAILOVER;
use super::{now_seconds, Live};
use crate::cmd;
use crate::model::Channel;
use crate::trial::{summarize, verdict, LiveHealth, Outcome, Thresholds, TrialSummary};

pub const TRIAL_SECONDS: u64 = 60;
pub const MIN_SECONDS: u64 = 20;
pub const MAX_SECONDS: u64 = 120;

impl Live {
    pub(crate) fn stop_trial_marker(&self) {
        let mut inner = self.lock();
        if inner.tests_running > 0 {
            inner.trial_cancel = true;
        }
    }

    pub async fn trial(&self, target: &Channel, seconds: u64) -> Result<TrialSummary, String> {
        let seconds = seconds.clamp(MIN_SECONDS, MAX_SECONDS);
        if matches!(self.lock().phase, crate::status::Phase::Paused | crate::status::Phase::Switching) {
            return Err("standbyOrSwitching".into());
        }
        let Ok(_gate) = self.gate.try_lock() else { return Err("switching".into()) };
        self.admission(Use::Trial, target.provider).map_err(|r| r.as_str().to_string())?;
        let source = self.source_for(target).map_err(|e| e.to_string())?;
        let settings = self.settings();
        let args = cmd::trial_args(&source, &settings.cmd_options());
        let child = Command::new(self.programs.ffmpeg.clone()).args(args).spawn().map_err(|e| e.to_string())?;
        {
            let mut inner = self.lock();
            inner.tests_running += 1;
            inner.trial_cancel = false;
        }
        drop(_gate);
        let mut blocks = Vec::new();
        let mut stderr_lines: Vec<String> = Vec::new();
        let mut carry = String::new();
        let began = Instant::now();
        let mut cancelled = false;
        while began.elapsed() < Duration::from_secs(seconds) {
            tokio::time::sleep(Duration::from_millis(500)).await;
            blocks.extend(media::lines(&mut carry, &child.read_stdout(1 << 16)).iter().filter_map(|l| media::out_time(l)));
            stderr_lines.extend(crate::portal::redact(&String::from_utf8_lossy(&child.read_stderr(1 << 16))).lines().map(str::to_string));
            if self.lock().trial_cancel {
                cancelled = true;
                break;
            }
            if child.try_wait().ok().flatten().is_some() {
                break;
            }
        }
        stop_child(&child).await;
        {
            let mut inner = self.lock();
            inner.tests_running = inner.tests_running.saturating_sub(1);
            if inner.tests_running == 0 {
                inner.trial_cancel = false;
            }
        }
        Ok(summarize(&blocks, &stderr_lines, cancelled))
    }

    pub(crate) fn launch_trial(self: &Arc<Self>, current: &Channel, _channel_name: Option<String>) {
        let rules = self.rules();
        let other = current.provider.other();
        let id = self.lock().policy.trial_id();
        let Some(target) = self.providers.mapped(current, other, &rules) else {
            self.lock().policy.trial_done(now_seconds(), Outcome::Impossible, "no equivalent channel", true, Some(id));
            return;
        };
        let live = self.clone();
        tokio::spawn(async move {
            let summary = live.trial(&target, TRIAL_SECONDS).await;
            let live_health = {
                let inner = live.lock();
                LiveHealth { state: inner.health.state.as_str().to_string(), loss_60s: inner.health.recent_loss(), restarts: inner.restarts }
            };
            let thresholds = {
                let settings = live.settings();
                Thresholds { gap_seconds_per_minute: settings.trial_gap, ratio: settings.trial_ratio }
            };
            let (outcome, reason, nothing) = match &summary {
                Ok(summary) => {
                    let v = verdict(&live_health, summary, &thresholds);
                    (v.outcome, v.reason, v.nothing_to_compare)
                }
                Err(error) => (Outcome::Impossible, error.clone(), false),
            };
            live.lock().policy.trial_done(now_seconds(), outcome, &reason, nothing, Some(id));
            live.host.publish(Event::new(TOPIC_FAILOVER, json!({ "kind": "trial", "outcome": outcome, "reason": reason, "to": other })));
        });
    }
}
