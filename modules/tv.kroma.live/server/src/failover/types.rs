use serde::{Deserialize, Serialize};

use crate::model::ProviderId;

pub const AUTO_COOLDOWN: f64 = 600.0;
pub const SUGGEST_COOLDOWN: f64 = 900.0;
pub const RECOVERY_CHECKS: u32 = 2;
pub const RETURN_RETRY: f64 = 300.0;
pub const TRIAL_COOLDOWN: f64 = 600.0;
pub const TRIAL_MAX: f64 = 240.0;
pub const PROPOSAL_VALID: f64 = 300.0;
pub const ALERT_COOLDOWN: f64 = 600.0;
pub const RECOVERED_AFTER: f64 = 20.0;
pub const VARIANT_MAX_SWITCHES: usize = 2;
pub const VARIANT_WINDOW: f64 = 600.0;
pub const VARIANT_STABLE: f64 = 120.0;
pub(crate) const VARIANT_MAX_FAMILY_NAMES: usize = 24;
pub(crate) const VARIANT_MAX_SWITCHES_KEPT: usize = 12;

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
