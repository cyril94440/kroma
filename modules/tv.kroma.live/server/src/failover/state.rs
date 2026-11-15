use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{VARIANT_MAX_FAMILY_NAMES, VARIANT_MAX_SWITCHES_KEPT};
use crate::model::ProviderId;
use crate::trial::Outcome;

const MAX_TEXT: usize = 300;

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelDown {
    pub channel: Option<String>,
    pub at: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Family {
    pub names: BTreeMap<String, f64>,
    pub switches: Vec<f64>,
    pub to: Option<String>,
    pub alerted_at: Option<f64>,
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
    pub channel_down: Option<ChannelDown>,
    pub variant_family: Option<Family>,
}

fn stamp_ok(value: f64, now: f64) -> bool {
    value.is_finite() && (0.0..=now + 60.0).contains(&value)
}

fn keep_stamp(slot: &mut Option<f64>, now: f64, name: &'static str, ignored: &mut Vec<&'static str>) {
    if slot.is_some_and(|value| !stamp_ok(value, now)) {
        *slot = None;
        ignored.push(name);
    }
}

fn keep_text(slot: &mut Option<String>, name: &'static str, ignored: &mut Vec<&'static str>) {
    if slot.as_ref().is_some_and(|text| text.chars().count() > MAX_TEXT) {
        *slot = None;
        ignored.push(name);
    }
}

fn family_valid(family: &Family, now: f64) -> bool {
    family.names.len() <= VARIANT_MAX_FAMILY_NAMES
        && family.switches.len() <= VARIANT_MAX_SWITCHES_KEPT
        && family.switches.iter().all(|at| stamp_ok(*at, now))
        && family.names.iter().all(|(name, at)| name.chars().count() <= MAX_TEXT && stamp_ok(*at, now))
        && family.to.as_ref().is_none_or(|to| to.chars().count() <= MAX_TEXT)
        && family.alerted_at.is_none_or(|at| stamp_ok(at, now))
}

macro_rules! load_fields {
    ($map:ident, $out:ident, $ignored:ident, $($key:literal => $field:ident),* $(,)?) => {
        $(
            if let Some(value) = $map.remove($key) {
                match serde_json::from_value(value) {
                    Ok(parsed) => $out.$field = parsed,
                    Err(_) => $ignored.push($key),
                }
            }
        )*
    };
}

impl Persisted {
    pub fn from_json(text: &str) -> (Persisted, Vec<&'static str>) {
        let mut out = Persisted::default();
        let mut ignored = Vec::new();
        let Ok(serde_json::Value::Object(mut map)) = serde_json::from_str(text) else {
            return (out, vec!["unreadable"]);
        };
        load_fields!(map, out, ignored,
            "primaryDownSince" => primary_down_since,
            "lastAutoAt" => last_auto_at,
            "lastSuggestAt" => last_suggest_at,
            "switchedAt" => switched_at,
            "channelAtSwitch" => channel_at_switch,
            "programAtSwitch" => program_at_switch,
            "returnSuggestedFor" => return_suggested_for,
            "bothDownAlerted" => both_down_alerted,
            "lastBothDownAt" => last_both_down_at,
            "lastRecoveredAt" => last_recovered_at,
            "lastTrialAt" => last_trial_at,
            "trialLive" => trial_live,
            "trialResult" => trial_result,
            "channelDown" => channel_down,
            "variantFamily" => variant_family,
        );
        (out, ignored)
    }

    pub fn cleaned(mut self, now: f64) -> (Persisted, Vec<&'static str>) {
        let mut ignored = Vec::new();
        keep_stamp(&mut self.primary_down_since, now, "primaryDownSince", &mut ignored);
        keep_stamp(&mut self.last_auto_at, now, "lastAutoAt", &mut ignored);
        keep_stamp(&mut self.last_suggest_at, now, "lastSuggestAt", &mut ignored);
        keep_stamp(&mut self.switched_at, now, "switchedAt", &mut ignored);
        keep_stamp(&mut self.last_both_down_at, now, "lastBothDownAt", &mut ignored);
        keep_stamp(&mut self.last_recovered_at, now, "lastRecoveredAt", &mut ignored);
        keep_stamp(&mut self.last_trial_at, now, "lastTrialAt", &mut ignored);
        keep_text(&mut self.channel_at_switch, "channelAtSwitch", &mut ignored);
        keep_text(&mut self.program_at_switch, "programAtSwitch", &mut ignored);
        keep_text(&mut self.return_suggested_for, "returnSuggestedFor", &mut ignored);
        if self.channel_down.as_ref().is_some_and(|down| !stamp_ok(down.at, now) || down.channel.as_ref().is_some_and(|c| c.chars().count() > MAX_TEXT)) {
            self.channel_down = None;
            ignored.push("channelDown");
        }
        if self.variant_family.as_ref().is_some_and(|family| !family_valid(family, now)) {
            self.variant_family = None;
            ignored.push("variantFamily");
        }
        if self.trial_result.as_ref().is_some_and(|result| !stamp_ok(result.at, now)) {
            self.trial_result = None;
            ignored.push("trialResult");
        }
        (self, ignored)
    }
}
