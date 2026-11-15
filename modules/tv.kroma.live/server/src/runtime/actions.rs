use std::sync::Arc;

use serde_json::json;

use super::notify::{BOTH_DOWN, FAILBACK, FAILOVER, RECOVERED};
use super::snapshot::{TOPIC_FAILOVER, TOPIC_PROPOSAL};
use super::switch::SwitchRequest;
use super::{now_ms, Live};
use crate::db::{self, ProviderEvent};
use crate::failover::{Action, Observation};
use crate::model::{Channel, ProviderId};
use crate::status::Proposal;

pub const PROPOSAL_SECONDS: i64 = 30;
pub const PARTY_SAFETY_SECONDS: i64 = 900;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Proposed {
    Voting,
    Applied,
    Declined,
}

fn all_busy(reasons: &[String]) -> bool {
    !reasons.is_empty() && reasons.iter().all(|r| r == "busy" || r == "liveAtThisProvider")
}

impl Live {
    pub(crate) async fn run_action(self: &Arc<Self>, action: Action, observation: &Observation, began: Option<f64>) {
        let Some(current) = self.current() else { return };
        let now = observation.now;
        let idle_action = matches!(action, Action::ReturnIdle | Action::Recovered | Action::BothDown);
        if !idle_action && !self.still_live(&current) {
            return;
        }
        if !matches!(action, Action::Failover | Action::Failback) {
            self.lock().policy.acknowledge(action, observation);
        }
        match action {
            Action::Failover => self.do_failover(&current, now, began).await,
            Action::Failback => self.do_failback(&current, now, began).await,
            Action::ReturnIdle => self.return_idle(&current).await,
            Action::BothDown => {
                self.emit(TOPIC_FAILOVER, json!({ "kind": "bothDown", "channel": current.name }));
                self.notify_people(BOTH_DOWN, &current.name, None);
            }
            Action::Recovered => {
                self.emit(TOPIC_FAILOVER, json!({ "kind": "recovered", "channel": current.name }));
                self.notify_people(RECOVERED, &current.name, None);
            }
            Action::SuggestBackup => self.suggest_backup(&current).await,
            Action::SuggestReturn => self.propose_for("returnToPrimary", &current, ProviderId::Primary, "programmeEnded").await,
            Action::ProposeTrial => self.propose_for("switchToBackup", &current, current.provider.other(), "trialBetter").await,
            Action::ReportTrial => self.emit(TOPIC_FAILOVER, json!({ "kind": "trialNotBetter", "channel": current.name })),
            Action::TrialBackup | Action::TrialPrimary => self.launch_trial(&current, observation.channel.clone()),
        }
    }

    async fn do_failover(self: &Arc<Self>, current: &Channel, now: f64, began: Option<f64>) {
        self.stop_trial_marker();
        if self.canary_alive(Some(current)).await {
            return self.channel_down_locally(current, now, began).await;
        }
        let (settings, rules) = (self.settings(), self.rules());
        let candidates = self.providers.equivalents(current, ProviderId::Backup, 3, &rules);
        let reachable = !candidates.is_empty() && self.providers.ping(ProviderId::Backup, &settings);
        let mut found = None;
        let mut reasons = Vec::new();
        if reachable {
            match self.working_equivalent(current, ProviderId::Backup).await {
                Ok(channel) => found = Some(channel),
                Err(tried) => reasons = tried,
            }
        }
        if found.is_none() && all_busy(&reasons) {
            return;
        }
        let Some(target) = found else {
            let returned = {
                let mut inner = self.lock();
                let returned = inner.policy.media_returned(began);
                if !returned {
                    inner.policy.skip_failover(now);
                    if !candidates.is_empty() && !reachable {
                        inner.policy.did_alert_both_down(now);
                    }
                }
                returned
            };
            if returned {
                return;
            }
            let kind = if candidates.is_empty() {
                "noEquivalent"
            } else if !reachable {
                "backupUnreachable"
            } else {
                "backupDead"
            };
            let _ = db::add_provider_event(&self.pool, &ProviderEvent { at: now_ms(), kind: "failoverSkipped".into(), channel: Some(current.name.clone()), detail: kind.into(), ..ProviderEvent::default() });
            self.emit(TOPIC_FAILOVER, json!({ "kind": kind, "channel": current.name, "to": ProviderId::Backup }));
            if kind == "backupUnreachable" {
                self.notify_people(BOTH_DOWN, &current.name, None);
            }
            return;
        };
        if !self.still_live(current) {
            return;
        }
        let returned = {
            let mut inner = self.lock();
            let returned = inner.policy.media_returned(began);
            if !returned {
                inner.policy.did_failover(now);
            }
            returned
        };
        if !returned {
            self.apply_move(current, &target, "failover", "deadPrimary", false).await;
        }
    }

    async fn do_failback(self: &Arc<Self>, current: &Channel, now: f64, began: Option<f64>) {
        let before = {
            let mut inner = self.lock();
            let before = inner.policy.saved.last_auto_at;
            inner.policy.did_failback(now);
            before
        };
        let (settings, rules) = (self.settings(), self.rules());
        let absent = self.providers.equivalents(current, ProviderId::Primary, 1, &rules).is_empty();
        let target = if self.providers.ping(ProviderId::Primary, &settings) { self.working_equivalent(current, ProviderId::Primary).await.ok() } else { None };
        let restore = |live: &Live| live.lock().policy.saved.last_auto_at = before;
        if !self.still_live(current) || self.lock().policy.media_returned(began) {
            return restore(self);
        }
        let Some(target) = target else {
            if !self.canary_alive(None).await {
                self.lock().policy.mark_primary_down(now);
                return;
            }
            let returned = {
                let mut inner = self.lock();
                let returned = inner.policy.media_returned(began);
                inner.policy.saved.last_auto_at = before;
                if !returned {
                    inner.policy.did_channel_down(now, Some(&current.name), None);
                }
                returned
            };
            if !returned {
                let reason = if absent { "absentFromPrimary" } else { "cutAtPrimaryToo" };
                self.emit(TOPIC_FAILOVER, json!({ "kind": "failbackRefused", "channel": current.name, "reason": reason }));
            }
            return;
        };
        self.apply_move(current, &target, "failback", "deadBackup", false).await;
    }

    async fn return_idle(self: &Arc<Self>, current: &Channel) {
        let settings = self.settings();
        if !self.providers.ping(ProviderId::Primary, &settings) {
            return;
        }
        let Ok(target) = self.working_equivalent(current, ProviderId::Primary).await else { return };
        self.apply_move(current, &target, "returnIdle", "primaryBack", true).await;
    }

    pub(crate) async fn apply_move(&self, current: &Channel, target: &Channel, kind: &str, reason: &str, only_if_paused: bool) {
        let outcome = self.switch(SwitchRequest { channel_id: target.id(), skip_if_same: true, only_if_paused, reason: reason.into(), ..SwitchRequest::default() }).await;
        if only_if_paused && outcome.status != "deferred" {
            return;
        }
        let _ = db::add_provider_event(
            &self.pool,
            &ProviderEvent { at: now_ms(), kind: kind.into(), from: Some(current.provider.as_str().into()), to: Some(target.provider.as_str().into()), channel: Some(current.name.clone()), detail: outcome.status.clone(), ..ProviderEvent::default() },
        );
        self.emit(TOPIC_FAILOVER, json!({ "kind": kind, "from": current.provider, "to": target.provider, "channel": current.name, "status": outcome.status }));
        match kind {
            "failover" => self.notify_people(FAILOVER, &current.name, Some(reason)),
            "failback" => self.notify_people(FAILBACK, &current.name, Some(reason)),
            _ => {}
        }
    }

    async fn suggest_backup(&self, current: &Channel) {
        match self.working_equivalent(current, ProviderId::Backup).await {
            Ok(target) => {
                self.propose_channel("switchToBackup", &target, "unstable", None).await;
            }
            Err(_) => self.emit(TOPIC_FAILOVER, json!({ "kind": "noWorkingEquivalent", "channel": current.name })),
        }
    }

    async fn propose_for(&self, kind: &str, current: &Channel, to: ProviderId, why: &str) {
        let rules = self.rules();
        let Some(mapped) = self.providers.mapped(current, to, &rules) else { return };
        self.propose_channel(kind, &mapped, why, None).await;
    }

    pub(crate) async fn propose_channel(&self, kind: &str, target: &Channel, why: &str, requested_by: Option<&str>) -> Proposed {
        let party = self.party_present();
        let window = if party { PARTY_SAFETY_SECONDS } else { PROPOSAL_SECONDS };
        let proposal = Proposal {
            id: kroma_module_sdk::primitives::random_token(),
            kind: kind.into(),
            text: why.into(),
            channel: Some(target.id()),
            requested_by: requested_by.map(str::to_string),
            created_at: now_ms(),
            expires_at: now_ms() + window * 1000,
        };
        if party {
            self.lock().proposals.insert(proposal.id.clone(), proposal.clone());
            self.emit(TOPIC_PROPOSAL, json!({ "proposal": proposal, "resolved": false, "viewers": self.viewer_count() }));
            return Proposed::Voting;
        }
        let accept = self.viewer_count() <= 1;
        self.emit(TOPIC_PROPOSAL, json!({ "proposal": proposal, "resolved": true, "accepted": accept, "default": "noVote" }));
        if accept && self.apply_proposal(&proposal).await { Proposed::Applied } else { Proposed::Declined }
    }

    async fn apply_proposal(&self, proposal: &Proposal) -> bool {
        let Some(channel_id) = proposal.channel.clone() else { return false };
        self.switch(SwitchRequest { channel_id, skip_if_same: true, reason: proposal.kind.clone(), ..SwitchRequest::default() }).await.switched()
    }

    pub async fn answer_proposal(&self, id: &str, accept: bool) -> bool {
        let Some(proposal) = self.lock().proposals.remove(id) else { return false };
        self.emit(TOPIC_PROPOSAL, json!({ "proposal": proposal, "resolved": true, "accepted": accept, "default": "vote" }));
        accept && self.apply_proposal(&proposal).await
    }

    pub(crate) fn expire_proposals(&self) {
        let now = now_ms();
        let party = self.party_present();
        let expired: Vec<Proposal> = {
            let mut inner = self.lock();
            let ids: Vec<String> = inner.proposals.iter().filter(|(_, p)| p.expires_at <= now || (!party && p.created_at + PROPOSAL_SECONDS * 1000 <= now)).map(|(id, _)| id.clone()).collect();
            ids.iter().filter_map(|id| inner.proposals.remove(id)).collect()
        };
        for proposal in expired {
            self.emit(TOPIC_PROPOSAL, json!({ "proposal": proposal, "resolved": true, "accepted": false, "default": "timeout" }));
        }
    }
}
