mod trial_logic;
#[cfg(test)]
mod tests;

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::model::ProviderId;
use crate::trial::Outcome;

pub const AUTO_COOLDOWN: f64 = 600.0;
pub const SUGGEST_COOLDOWN: f64 = 900.0;
pub const RECOVERY_CHECKS: u32 = 2;
pub const RETURN_RETRY: f64 = 300.0;
pub const TRIAL_COOLDOWN: f64 = 600.0;
pub const TRIAL_MAX: f64 = 240.0;
pub const PROPOSAL_VALID: f64 = 300.0;
pub const ALERT_COOLDOWN: f64 = 600.0;
pub const RECOVERED_AFTER: f64 = 20.0;

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub dead_after: f64,
    pub unstable_after: f64,
    pub unstable_window: f64,
    pub trial_after: f64,
    pub trouble_dead: f64,
}

impl Default for Limits {
    fn default() -> Self {
        Limits { dead_after: 45.0, unstable_after: 120.0, unstable_window: 300.0, trial_after: 20.0, trouble_dead: 20.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    Failover,
    SuggestBackup,
    Failback,
    BothDown,
    Recovered,
    ReturnIdle,
    SuggestReturn,
    TrialBackup,
    TrialPrimary,
    ProposeTrial,
    ReportTrial,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TrialRecord {
    pub outcome: Outcome,
    pub reason: String,
    pub at: f64,
    pub live: (Option<ProviderId>, Option<String>),
    pub proposed: bool,
    pub reported: bool,
}

impl Default for TrialRecord {
    fn default() -> Self {
        TrialRecord { outcome: Outcome::Impossible, reason: String::new(), at: 0.0, live: (None, None), proposed: false, reported: false }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Persisted {
    pub primary_down_since: Option<f64>,
    pub last_auto_at: Option<f64>,
    pub last_suggest_at: Option<f64>,
    pub switched_at: Option<f64>,
    pub channel_at_switch: Option<String>,
    pub program_at_switch: Option<String>,
    pub return_suggested_for: Option<String>,
    pub both_down_alerted: bool,
    pub last_both_down_at: Option<f64>,
    pub last_recovered_at: Option<f64>,
    pub last_trial_at: Option<f64>,
    pub trial_live: Option<(Option<ProviderId>, Option<String>)>,
    pub trial_result: Option<TrialRecord>,
}

#[derive(Debug, Clone)]
pub struct Observation {
    pub now: f64,
    pub provider: Option<ProviderId>,
    pub paused: bool,
    pub media: bool,
    pub unstable: bool,
    pub viewers: usize,
    pub program: Option<String>,
    pub channel: Option<String>,
}

#[derive(Debug)]
pub struct FailoverPolicy {
    pub has_backup: bool,
    pub limits: Limits,
    pub saved: Persisted,
    trial_seq: u64,
    provider: Option<ProviderId>,
    dead_since: Option<f64>,
    unstable_since: Option<f64>,
    media_since: Option<f64>,
    recovery_ok: u32,
    last_return_try: Option<f64>,
    bad_runs: VecDeque<(f64, f64)>,
    prev_obs: Option<(f64, bool)>,
    obs_channel: Option<String>,
}

pub fn elapsed(since: Option<f64>, now: f64, delay: f64) -> bool {
    since.is_none_or(|at| now - at >= delay)
}

impl FailoverPolicy {
    pub fn new(has_backup: bool, limits: Limits, saved: Persisted, now: f64) -> FailoverPolicy {
        let mut saved = saved;
        if saved.last_trial_at.is_some() && saved.trial_result.is_none() {
            match saved.trial_live.clone() {
                None => saved.last_trial_at = None,
                Some(live) => {
                    saved.trial_result = Some(TrialRecord { outcome: Outcome::Impossible, reason: "restart".into(), at: now, live, ..TrialRecord::default() })
                }
            }
        }
        FailoverPolicy {
            has_backup,
            limits,
            saved,
            trial_seq: 0,
            provider: None,
            dead_since: None,
            unstable_since: None,
            media_since: None,
            recovery_ok: 0,
            last_return_try: None,
            bad_runs: VecDeque::new(),
            prev_obs: None,
            obs_channel: None,
        }
    }

    pub fn trial_id(&self) -> u64 {
        self.trial_seq
    }

    pub fn primary_is_down(&self) -> bool {
        self.has_backup && self.saved.primary_down_since.is_some()
    }

    pub fn zap_provider(&self) -> ProviderId {
        if self.primary_is_down() { ProviderId::Backup } else { ProviderId::Primary }
    }

    pub fn trouble(&self, now: f64) -> bool {
        self.dead_since.is_some_and(|since| now - since >= self.limits.trouble_dead) || self.unstable_since.is_some()
    }

    fn note_unstable(&mut self, now: f64, bad: bool) {
        if let Some((at, was_bad)) = self.prev_obs {
            if was_bad && now > at {
                let start = self.bad_runs.back().map_or(at, |run| at.max(run.1));
                let end = now.min(at + 30.0);
                if end > start {
                    self.bad_runs.push_back((start, end));
                }
            }
        }
        self.prev_obs = Some((now, bad));
        while self.bad_runs.front().is_some_and(|run| run.1 <= now - self.limits.unstable_window) {
            self.bad_runs.pop_front();
        }
    }

    pub fn unstable_for(&self, now: f64) -> f64 {
        let current = self.unstable_since.map_or(0.0, |since| now - since);
        let start = now - self.limits.unstable_window;
        let cumulative: f64 = self.bad_runs.iter().map(|(b, e)| (e.min(now) - b.max(start)).max(0.0)).sum();
        current.max(cumulative)
    }

    fn on_provider_change(&mut self, now: f64, provider: Option<ProviderId>, program: &Option<String>, channel: &Option<String>) {
        self.provider = provider;
        self.dead_since = None;
        self.unstable_since = None;
        self.bad_runs.clear();
        self.prev_obs = None;
        match provider {
            Some(ProviderId::Backup) => {
                if self.saved.switched_at.is_none() {
                    self.saved.switched_at = Some(now);
                    self.saved.channel_at_switch = channel.clone();
                    self.saved.program_at_switch = program.clone();
                }
            }
            Some(ProviderId::Primary) => {
                self.saved.switched_at = None;
                self.saved.channel_at_switch = None;
                self.saved.program_at_switch = None;
                self.saved.return_suggested_for = None;
                self.saved.both_down_alerted = false;
            }
            None => {}
        }
    }

    pub fn observe(&mut self, obs: &Observation) -> Option<Action> {
        let now = obs.now;
        if obs.provider != self.provider {
            self.on_provider_change(now, obs.provider, &obs.program, &obs.channel);
        } else if obs.channel.is_some() && self.obs_channel.is_some() && obs.channel != self.obs_channel {
            self.bad_runs.clear();
            self.prev_obs = None;
            self.unstable_since = None;
            self.dead_since = None;
        }
        if obs.channel.is_some() {
            self.obs_channel = obs.channel.clone();
        }
        if obs.provider == Some(ProviderId::Backup) && obs.channel.is_some() && obs.channel != self.saved.channel_at_switch {
            self.saved.channel_at_switch = obs.channel.clone();
            self.saved.program_at_switch = obs.program.clone();
            self.saved.return_suggested_for = None;
        }
        let provider = obs.provider?;
        if !self.has_backup {
            return None;
        }
        if provider == ProviderId::Primary && obs.media && self.saved.primary_down_since.is_some() {
            self.saved.primary_down_since = None;
            self.recovery_ok = 0;
        }
        let live = !obs.paused;
        let dead = live && !obs.media;
        self.dead_since = if dead { self.dead_since.or(Some(now)) } else { None };
        self.media_since = if live && obs.media { self.media_since.or(Some(now)) } else { None };
        let bad = live && obs.media && obs.unstable;
        self.unstable_since = if bad { self.unstable_since.or(Some(now)) } else { None };
        self.note_unstable(now, bad);
        if self.saved.both_down_alerted && self.media_since.is_some_and(|since| now - since >= RECOVERED_AFTER) {
            self.saved.both_down_alerted = false;
            if obs.viewers > 0 && elapsed(self.saved.last_recovered_at, now, ALERT_COOLDOWN) {
                return Some(Action::Recovered);
            }
        }
        if obs.paused {
            if provider == ProviderId::Backup && self.saved.primary_down_since.is_none() && elapsed(self.last_return_try, now, RETURN_RETRY) {
                self.last_return_try = Some(now);
                return Some(Action::ReturnIdle);
            }
            return None;
        }
        if obs.viewers == 0 {
            return None;
        }
        let dead_long = self.dead_since.is_some_and(|since| now - since >= self.limits.dead_after);
        if provider == ProviderId::Primary {
            if dead_long && elapsed(self.saved.last_auto_at, now, AUTO_COOLDOWN) {
                return Some(Action::Failover);
            }
            if let Some(action) = self.trial_action(bad, now, provider, &obs.channel, Action::TrialBackup) {
                return Some(action);
            }
            if bad && self.unstable_for(now) >= self.limits.unstable_after && !self.trial_decides(now, provider, &obs.channel) && elapsed(self.saved.last_suggest_at, now, SUGGEST_COOLDOWN) {
                return Some(Action::SuggestBackup);
            }
            return None;
        }
        if dead_long {
            if self.saved.primary_down_since.is_none() && elapsed(self.saved.last_auto_at, now, AUTO_COOLDOWN) {
                return Some(Action::Failback);
            }
            if self.saved.primary_down_since.is_some() && !self.saved.both_down_alerted && elapsed(self.saved.last_both_down_at, now, ALERT_COOLDOWN) {
                return Some(Action::BothDown);
            }
            return None;
        }
        if self.saved.primary_down_since.is_none() {
            if let Some(action) = self.trial_action(bad, now, provider, &obs.channel, Action::TrialPrimary) {
                return Some(action);
            }
        }
        let ended = obs.program.is_some() && self.saved.program_at_switch.is_some() && obs.program != self.saved.program_at_switch && obs.program != self.saved.return_suggested_for;
        if self.saved.primary_down_since.is_none() && ended && elapsed(self.saved.last_suggest_at, now, SUGGEST_COOLDOWN) {
            return Some(Action::SuggestReturn);
        }
        None
    }

    pub fn acknowledge(&mut self, action: Action, obs: &Observation) {
        let now = obs.now;
        match action {
            Action::Failover => self.did_failover(now),
            Action::SuggestBackup => self.did_suggest(now),
            Action::Failback => self.did_failback(now),
            Action::BothDown => self.did_alert_both_down(now),
            Action::Recovered => self.did_recovered(now),
            Action::SuggestReturn => self.did_suggest_return(now, obs.program.clone()),
            Action::TrialBackup => {
                self.did_trial(now, ProviderId::Primary, obs.channel.clone());
            }
            Action::TrialPrimary => {
                self.did_trial(now, ProviderId::Backup, obs.channel.clone());
            }
            Action::ProposeTrial => self.did_use_trial(now, true),
            Action::ReportTrial => self.did_use_trial(now, false),
            Action::ReturnIdle => {}
        }
    }

    pub fn did_failover(&mut self, now: f64) {
        self.saved.last_auto_at = Some(now);
        self.saved.primary_down_since.get_or_insert(now);
        self.recovery_ok = 0;
    }

    pub fn did_suggest(&mut self, now: f64) {
        self.saved.last_suggest_at = Some(now);
    }

    pub fn did_suggest_return(&mut self, now: f64, program: Option<String>) {
        self.saved.return_suggested_for = program;
        self.saved.last_suggest_at = Some(now);
    }

    pub fn did_failback(&mut self, now: f64) {
        self.saved.last_auto_at = Some(now);
    }

    pub fn skip_failover(&mut self, now: f64) {
        self.saved.last_auto_at = Some(now);
        self.mark_primary_down(now);
    }

    pub fn mark_primary_down(&mut self, now: f64) {
        self.saved.primary_down_since.get_or_insert(now);
        self.recovery_ok = 0;
    }

    pub fn did_alert_both_down(&mut self, now: f64) {
        self.saved.both_down_alerted = true;
        self.saved.last_both_down_at = Some(now);
    }

    pub fn did_recovered(&mut self, now: f64) {
        self.saved.last_recovered_at = Some(now);
    }

    pub fn probe_primary(&mut self, ok: bool) -> bool {
        if self.saved.primary_down_since.is_none() {
            return false;
        }
        if !ok {
            self.recovery_ok = 0;
            return false;
        }
        self.recovery_ok += 1;
        if self.recovery_ok >= RECOVERY_CHECKS {
            self.saved.primary_down_since = None;
            self.saved.both_down_alerted = false;
            self.recovery_ok = 0;
            return true;
        }
        false
    }
}
