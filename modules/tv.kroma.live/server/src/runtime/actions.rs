use std::time::{Duration, Instant};

use kroma_module_sdk::host::Event;
use serde_json::json;

use super::snapshot::{TOPIC_FAILOVER, TOPIC_PROPOSAL};
use super::switch::SwitchRequest;
use super::{now_ms, now_seconds, Live, KV_POLICY};
use crate::db::{self, ProviderEvent};
use crate::failover::{Action, Observation};
use crate::health::State;
use crate::model::{Channel, ProviderId};
use crate::status::{Phase, Proposal};

const OBSERVE_EVERY: Duration = Duration::from_secs(10);
pub const PROPOSAL_SECONDS: i64 = 30;

impl Live {
    fn observation(&self) -> Option<Observation> {
        let inner = self.lock();
        let channel = inner.current.clone()?;
        let media = inner.run.as_ref().is_some_and(|r| r.child.try_wait().ok().flatten().is_none()) && inner.kbps >= self.settings().min_kbps;
        let unstable = matches!(inner.health.state, State::Degraded | State::Severe) || inner.restarts >= 2;
        let (program, _) = super::now_next(&self.pool, channel.epg_id.as_deref(), &channel.name, now_ms());
        Some(Observation {
            now: now_seconds(),
            provider: Some(channel.provider),
            paused: inner.phase == Phase::Paused,
            media,
            unstable,
            viewers: inner.presence.count(),
            program: program.map(|p| p.title),
            channel: Some(channel.name),
        })
    }

    pub(crate) async fn failover_step(self: &std::sync::Arc<Self>) {
        {
            let mut inner = self.lock();
            if inner.last_observe.is_some_and(|at| at.elapsed() < OBSERVE_EVERY) || inner.phase == Phase::Switching {
                return;
            }
            inner.last_observe = Some(Instant::now());
        }
        self.expire_proposals();
        self.check_primary_recovery();
        let Some(observation) = self.observation() else { return };
        let settings = self.settings();
        let action = {
            let mut inner = self.lock();
            inner.policy.has_backup = settings.has_backup();
            inner.policy.limits = super::limits_of(&settings);
            let action = inner.policy.observe(&observation);
            if let Some(action) = action {
                inner.policy.acknowledge(action, &observation);
            }
            action
        };
        if let Some(action) = action {
            if settings.backup_enabled_failover || !matches!(action, Action::Failover) {
                self.run_action(action, &observation).await;
            }
        }
        self.save_policy();
    }

    fn check_primary_recovery(&self) {
        let waiting = {
            let inner = self.lock();
            let on_backup = inner.current.as_ref().is_some_and(|c| c.provider == ProviderId::Backup);
            inner.policy.primary_is_down() && on_backup && inner.last_ping.is_none_or(|at| at.elapsed() >= Duration::from_secs(30))
        };
        if !waiting {
            return;
        }
        self.lock().last_ping = Some(Instant::now());
        let ok = self.providers.ping(ProviderId::Primary, &self.settings());
        if self.lock().policy.probe_primary(ok) {
            self.host.publish(Event::new(TOPIC_FAILOVER, json!({ "kind": "primaryBack" })));
        }
    }

    fn save_policy(&self) {
        let mut inner = self.lock();
        let Ok(json) = serde_json::to_string(&inner.policy.saved) else { return };
        if inner.saved_policy_json != json {
            inner.saved_policy_json = json.clone();
            drop(inner);
            let _ = db::kv_set(&self.pool, KV_POLICY, &json);
        }
    }

    pub(crate) async fn run_action(self: &std::sync::Arc<Self>, action: Action, observation: &Observation) {
        let Some(current) = self.current() else { return };
        match action {
            Action::Failover => self.move_to(&current, ProviderId::Backup, "failover", "deadPrimary", false).await,
            Action::Failback => self.move_to(&current, ProviderId::Primary, "failback", "deadBackup", false).await,
            Action::ReturnIdle => self.move_to(&current, ProviderId::Primary, "returnIdle", "primaryBack", true).await,
            Action::BothDown => self.host.publish(Event::new(TOPIC_FAILOVER, json!({ "kind": "bothDown", "channel": current.name }))),
            Action::Recovered => self.host.publish(Event::new(TOPIC_FAILOVER, json!({ "kind": "recovered", "channel": current.name }))),
            Action::SuggestBackup => self.propose("switchToBackup", &current, ProviderId::Backup, "unstable").await,
            Action::SuggestReturn => self.propose("returnToPrimary", &current, ProviderId::Primary, "programmeEnded").await,
            Action::ProposeTrial => self.propose("switchToBackup", &current, current.provider.other(), "trialBetter").await,
            Action::ReportTrial => self.host.publish(Event::new(TOPIC_FAILOVER, json!({ "kind": "trialNotBetter", "channel": current.name }))),
            Action::TrialBackup | Action::TrialPrimary => self.launch_trial(&current, observation.channel.clone()),
        }
    }

    async fn move_to(&self, current: &Channel, to: ProviderId, kind: &str, reason: &str, only_if_paused: bool) {
        let rules = self.rules();
        let Some(mapped) = self.providers.mapped(current, to, &rules) else {
            self.lock().policy.skip_failover(now_seconds());
            self.host.publish(Event::new(TOPIC_FAILOVER, json!({ "kind": "noEquivalent", "channel": current.name, "to": to })));
            return;
        };
        let outcome = self.switch(SwitchRequest { channel_id: mapped.id(), skip_if_same: true, only_if_paused, reason: reason.into(), ..SwitchRequest::default() }).await;
        let _ = db::add_provider_event(
            &self.pool,
            &ProviderEvent { at: now_ms(), kind: kind.into(), from: Some(current.provider.as_str().into()), to: Some(to.as_str().into()), channel: Some(current.name.clone()), detail: outcome.status.clone(), ..ProviderEvent::default() },
        );
        self.host.publish(Event::new(TOPIC_FAILOVER, json!({ "kind": kind, "from": current.provider, "to": to, "channel": current.name, "status": outcome.status })));
    }

    async fn propose(&self, kind: &str, current: &Channel, to: ProviderId, why: &str) {
        let rules = self.rules();
        let Some(mapped) = self.providers.mapped(current, to, &rules) else { return };
        let proposal = Proposal {
            id: kroma_module_sdk::primitives::random_token(),
            kind: kind.into(),
            text: why.into(),
            channel: Some(mapped.id()),
            created_at: now_ms(),
            expires_at: now_ms() + PROPOSAL_SECONDS * 1000,
        };
        if self.party_present() {
            self.lock().proposals.insert(proposal.id.clone(), proposal.clone());
            self.host.publish(Event::new(TOPIC_PROPOSAL, json!({ "proposal": proposal, "resolved": false, "viewers": self.viewer_count() })));
            return;
        }
        let accept = self.viewer_count() <= 1;
        self.host.publish(Event::new(TOPIC_PROPOSAL, json!({ "proposal": proposal, "resolved": true, "accepted": accept, "default": "noVote" })));
        if accept {
            self.apply_proposal(&proposal).await;
        }
    }

    async fn apply_proposal(&self, proposal: &Proposal) -> bool {
        let Some(channel_id) = proposal.channel.clone() else { return false };
        self.switch(SwitchRequest { channel_id, skip_if_same: true, reason: proposal.kind.clone(), ..SwitchRequest::default() }).await.switched()
    }

    pub async fn answer_proposal(&self, id: &str, accept: bool) -> bool {
        let Some(proposal) = self.lock().proposals.remove(id) else { return false };
        self.host.publish(Event::new(TOPIC_PROPOSAL, json!({ "proposal": proposal, "resolved": true, "accepted": accept, "default": "vote" })));
        accept && self.apply_proposal(&proposal).await
    }

    fn expire_proposals(&self) {
        let now = now_ms();
        let expired: Vec<Proposal> = {
            let mut inner = self.lock();
            let ids: Vec<String> = inner.proposals.iter().filter(|(_, p)| p.expires_at <= now).map(|(id, _)| id.clone()).collect();
            ids.iter().filter_map(|id| inner.proposals.remove(id)).collect()
        };
        for proposal in expired {
            self.host.publish(Event::new(TOPIC_PROPOSAL, json!({ "proposal": proposal, "resolved": true, "accepted": false, "default": "timeout" })));
        }
    }
}
