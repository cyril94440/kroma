use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::json;

use super::snapshot::TOPIC_FAILOVER;
use super::{now_ms, now_seconds, Live, KV_POLICY};
use crate::db;
use crate::failover::trace::TraceRecord;
use crate::failover::{Action, Observation};
use crate::health::State;
use crate::model::ProviderId;
use crate::status::Phase;

const OBSERVE_EVERY: Duration = Duration::from_secs(10);
const PRIMARY_RECHECK_EVERY: Duration = Duration::from_secs(30);

impl Live {
    fn observation(&self) -> Option<Observation> {
        let min_kbps = self.settings().min_kbps;
        let mut inner = self.lock();
        let channel = inner.current.clone()?;
        let running = inner.run.as_ref().is_some_and(|r| r.child.try_wait().ok().flatten().is_none());
        let healthy = !inner.black && !inner.frozen && !inner.health.frozen;
        let kbps = inner.kbps;
        let media = inner.bitrate.flowing(running, kbps, Some(&channel.name), healthy, min_kbps);
        let stuttering = matches!(inner.health.state, State::Degraded | State::Severe) && inner.health.cause() != crate::health::Cause::Local;
        let unstable = stuttering || inner.restarts >= 2;
        let paused = inner.phase == Phase::Paused;
        let viewers = inner.presence.count();
        drop(inner);
        let (program, _) = super::now_next(&self.pool, channel.epg_id.as_deref(), &channel.name, now_ms());
        Some(Observation { now: now_seconds(), provider: Some(channel.provider), paused, media, unstable, viewers, program: program.map(|p| p.title), channel: Some(channel.name) })
    }

    fn trace_observation(&self, observation: &Observation, action: Option<Action>, began: Option<f64>, unstable_since: Option<f64>) {
        let mut inner = self.lock();
        let record = TraceRecord {
            provider: observation.provider,
            paused: observation.paused,
            media: observation.media,
            unstable: observation.unstable,
            viewers: observation.viewers,
            program: observation.program.clone(),
            channel: observation.channel.clone(),
            action,
            source_kbps: Some(inner.kbps),
            kbps_median: inner.bitrate.median(),
            restarts: Some(inner.restarts),
            loss_60s: Some(inner.health.recent_loss()),
            health: Some(inner.health.state.as_str().to_string()),
            dead_since: began,
            unstable_since,
        };
        let trouble = began.is_some() || unstable_since.is_some() || action.is_some();
        inner.trace.record(observation.now, &record, trouble);
    }

    pub(crate) async fn failover_step(self: &Arc<Self>) {
        {
            let mut inner = self.lock();
            if inner.last_observe.is_some_and(|at| at.elapsed() < OBSERVE_EVERY) || inner.phase == Phase::Switching {
                return;
            }
            inner.last_observe = Some(Instant::now());
        }
        self.expire_proposals();
        let Some(observation) = self.observation() else { return };
        let settings = self.settings();
        let (action, began, unstable_since) = {
            let mut inner = self.lock();
            inner.policy.has_backup = settings.has_backup();
            inner.policy.limits = super::limits_of(&settings);
            let action = inner.policy.observe(&observation);
            (action, inner.policy.dead_since(), inner.policy.unstable_since())
        };
        self.trace_observation(&observation, action, began, unstable_since);
        self.spawn_primary_recheck();
        if let Some(action) = action.filter(|a| settings.backup_enabled_failover || *a != Action::Failover) {
            self.dispatch(action, observation, began);
        }
        self.save_policy();
    }

    fn dispatch(self: &Arc<Self>, action: Action, observation: Observation, began: Option<f64>) {
        let dropped = {
            let mut inner = self.lock();
            let busy = inner.action_busy;
            inner.action_busy = true;
            if busy {
                inner.policy.undo(action);
            }
            busy
        };
        if dropped {
            return;
        }
        let live = self.clone();
        tokio::spawn(async move {
            live.run_action(action, &observation, began).await;
            live.lock().action_busy = false;
            live.save_policy();
            live.publish_status();
        });
    }

    fn spawn_primary_recheck(self: &Arc<Self>) {
        let due = {
            let mut inner = self.lock();
            let on_backup = inner.current.as_ref().is_some_and(|c| c.provider == ProviderId::Backup);
            let due = inner.policy.primary_is_down() && on_backup && !inner.recheck_busy && inner.last_ping.is_none_or(|at| at.elapsed() >= PRIMARY_RECHECK_EVERY);
            if due {
                inner.last_ping = Some(Instant::now());
                inner.recheck_busy = true;
            }
            due
        };
        if !due {
            return;
        }
        let live = self.clone();
        tokio::spawn(async move {
            let healthy = live.primary_healthy().await;
            let recovered = live.lock().policy.probe_primary(healthy);
            if recovered {
                live.emit(TOPIC_FAILOVER, json!({ "kind": "primaryBack" }));
            }
            live.lock().recheck_busy = false;
        });
    }

    async fn primary_healthy(&self) -> bool {
        let settings = self.settings();
        if !self.providers.ping(ProviderId::Primary, &settings) {
            return false;
        }
        let Some(current) = self.current() else { return true };
        if self.providers.equivalents(&current, ProviderId::Primary, 1, &self.rules()).is_empty() {
            return true;
        }
        self.working_equivalent(&current, ProviderId::Primary).await.is_ok() || self.canary_alive(None).await
    }

    pub(crate) fn save_policy(&self) {
        let mut inner = self.lock();
        let Ok(json) = serde_json::to_string(&inner.policy.saved) else { return };
        if inner.saved_policy_json != json {
            inner.saved_policy_json = json.clone();
            drop(inner);
            let _ = db::kv_set(&self.pool, KV_POLICY, &json);
        }
    }
}
