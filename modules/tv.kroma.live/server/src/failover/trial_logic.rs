use super::*;

impl FailoverPolicy {
    pub(super) fn trial_in_flight(&self, now: f64) -> bool {
        self.saved.last_trial_at.is_some_and(|at| self.saved.trial_result.is_none() && now - at < TRIAL_MAX)
    }

    pub(super) fn result_for(&self, provider: ProviderId, channel: &Option<String>, now: f64) -> Option<&TrialRecord> {
        self.saved.trial_result.as_ref().filter(|r| r.live == (Some(provider), channel.clone()) && now - r.at < TRIAL_COOLDOWN)
    }

    pub(super) fn trial_action(&self, bad: bool, now: f64, provider: ProviderId, channel: &Option<String>, launch: Action) -> Option<Action> {
        if !bad {
            return None;
        }
        let unstable_since = self.unstable_since?;
        if let Some(result) = self.result_for(provider, channel, now) {
            if elapsed(self.saved.last_suggest_at, now, SUGGEST_COOLDOWN) {
                if result.outcome == Outcome::Better && !result.proposed && now - result.at < PROPOSAL_VALID && self.saved.last_auto_at.unwrap_or(0.0) < result.at {
                    return Some(Action::ProposeTrial);
                }
                if result.outcome == Outcome::NotBetter && !result.reported && self.unstable_for(now) >= self.limits.unstable_after {
                    return Some(Action::ReportTrial);
                }
            }
        }
        let same_live = self.saved.trial_live.as_ref() == Some(&(Some(provider), channel.clone()));
        if now - unstable_since >= self.limits.trial_after
            && !self.trial_in_flight(now)
            && (!same_live || elapsed(self.saved.last_trial_at, now, TRIAL_COOLDOWN))
            && elapsed(self.saved.last_suggest_at, now, SUGGEST_COOLDOWN)
        {
            return Some(launch);
        }
        None
    }

    pub(super) fn trial_decides(&self, now: f64, provider: ProviderId, channel: &Option<String>) -> bool {
        if self.trial_in_flight(now) {
            return true;
        }
        match self.result_for(provider, channel, now) {
            Some(r) if r.outcome == Outcome::NotBetter => true,
            Some(r) if r.outcome == Outcome::Better => r.proposed || now - r.at < PROPOSAL_VALID,
            _ => false,
        }
    }

    pub fn did_trial(&mut self, now: f64, provider: ProviderId, channel: Option<String>) -> u64 {
        self.saved.last_trial_at = Some(now);
        self.saved.trial_live = Some((Some(provider), channel));
        self.saved.trial_result = None;
        self.trial_seq += 1;
        self.trial_seq
    }

    pub fn trial_done(&mut self, now: f64, outcome: Outcome, reason: &str, nothing_to_compare: bool, trial_id: Option<u64>) -> bool {
        if trial_id.is_some_and(|id| id != self.trial_seq) {
            return false;
        }
        if nothing_to_compare {
            self.saved.last_trial_at = None;
        }
        let live = self.saved.trial_live.clone().unwrap_or((None, None));
        self.saved.trial_result = Some(TrialRecord { outcome, reason: crate::portal::redact(reason), at: now, live, proposed: false, reported: false });
        true
    }

    pub fn did_use_trial(&mut self, now: f64, proposed: bool) {
        if let Some(result) = self.saved.trial_result.as_mut() {
            if proposed { result.proposed = true } else { result.reported = true }
        }
        self.saved.last_suggest_at = Some(now);
    }
}
