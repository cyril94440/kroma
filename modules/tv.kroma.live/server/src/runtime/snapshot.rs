use kroma_module_sdk::host::Event;
use serde_json::json;

use super::guide::now_next;
use super::{now_ms, Live};
use crate::cmd::PLAYLIST_FILE;
use crate::health::State;
use crate::model::ProviderId;
use crate::status::{ChannelRef, HealthView, Phase, Slot, Status};

pub const TOPIC_STATUS: &str = "live.status";
pub const TOPIC_CHANNEL: &str = "live.channel";
pub const TOPIC_PRESENCE: &str = "live.presence";
pub const TOPIC_HEALTH: &str = "live.health";
pub const TOPIC_PROPOSAL: &str = "live.proposal";
pub const TOPIC_FAILOVER: &str = "live.failover";
pub const TOPIC_SCAN: &str = "live.scan";

impl Live {
    pub fn status(&self) -> Status {
        let settings = self.settings();
        let now = now_ms();
        let (channel, phase) = {
            let inner = self.lock();
            (inner.current.clone(), inner.phase)
        };
        let (current, next) = channel
            .as_ref()
            .map(|c| now_next(&self.pool, c.epg_id.as_deref(), &c.name, now))
            .unwrap_or((None, None));
        let inner = self.lock();
        let standby = inner.presence.idle_for_ms(now).filter(|_| phase != Phase::Paused).map(|idle| (settings.standby_seconds as i64 - idle / 1000).max(0) as u64);
        let playlist = inner.run.as_ref().filter(|_| phase != Phase::Paused).map(|run| format!("{}/{PLAYLIST_FILE}", run.folder));
        Status {
            state: phase,
            channel: channel.as_ref().map(|c| ChannelRef { id: c.id(), name: c.name.clone() }),
            provider: channel.as_ref().map_or(ProviderId::Primary, |c| c.provider),
            on_backup: channel.as_ref().is_some_and(|c| c.provider == ProviderId::Backup),
            now: current.as_ref().map(Slot::of),
            next: next.as_ref().map(Slot::of),
            viewers: inner.presence.viewers(),
            health: health_view(&inner),
            playlist,
            started_at: inner.started_wall,
            restarts: inner.restarts_total,
            standby_in_seconds: standby,
            last_error: inner.last_error.clone(),
            off_air: inner.off_air.clone(),
            backup_configured: settings.has_backup(),
            primary_down: inner.policy.primary_is_down(),
        }
    }

    pub fn publish_status(&self) {
        let status = self.status();
        let Ok(json) = serde_json::to_string(&status) else { return };
        {
            let mut inner = self.lock();
            if inner.last_status_json == json {
                return;
            }
            inner.last_status_json = json;
        }
        self.host.publish(Event::new(TOPIC_STATUS, serde_json::to_value(&status).unwrap_or_default()));
    }

    pub fn publish_channel(&self) {
        let status = self.status();
        self.host.publish(Event::new(
            TOPIC_CHANNEL,
            json!({ "channel": status.channel, "provider": status.provider, "onBackup": status.on_backup, "now": status.now, "next": status.next, "playlist": status.playlist }),
        ));
    }

    pub fn publish_presence(&self) {
        let viewers = self.lock().presence.viewers();
        self.host.publish(Event::new(TOPIC_PRESENCE, json!({ "viewers": viewers })));
    }

    pub fn publish_health(&self, force: bool) {
        let view = health_view(&self.lock());
        let Ok(json) = serde_json::to_string(&view) else { return };
        {
            let mut inner = self.lock();
            if !force && inner.last_health_json == json {
                return;
            }
            inner.last_health_json = json;
        }
        self.host.publish(Event::new(TOPIC_HEALTH, serde_json::to_value(&view).unwrap_or_default()));
    }
}

pub fn health_view(inner: &super::Inner) -> HealthView {
    HealthView {
        source_kbps: (inner.kbps * 10.0).round() / 10.0,
        stutter: matches!(inner.health.state, State::Degraded | State::Severe),
        black: inner.black,
        frozen: inner.frozen || inner.health.frozen,
        trouble: inner.policy.trouble(super::now_seconds()),
        state: inner.health.state.as_str().to_string(),
        loss_seconds: inner.health.recent_loss(),
    }
}
