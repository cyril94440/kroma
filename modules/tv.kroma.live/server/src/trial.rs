use serde::{Deserialize, Serialize};

use crate::health::{StreamHealth, CATCH_UP_MARGIN, GRACE_BLOCKS, OPENING_THRESHOLD};

fn rises(lags: &[f64], threshold: f64) -> f64 {
    let Some(first) = lags.first().copied() else { return 0.0 };
    let (mut total, mut low, mut high, mut climbing) = (0.0, first, first, true);
    for lag in &lags[1..] {
        if climbing {
            if *lag > high {
                high = *lag;
            } else if high - lag >= threshold {
                if high - low >= threshold {
                    total += high - low;
                }
                climbing = false;
                low = *lag;
            }
        } else if *lag < low {
            low = *lag;
        } else if lag - low >= threshold {
            climbing = true;
            high = *lag;
        }
    }
    if climbing && high - low >= threshold {
        total += high - low;
    }
    total
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TrialSummary {
    pub ok: bool,
    pub cancelled: bool,
    pub blocks: usize,
    pub measured_seconds: usize,
    pub loss_seconds: f64,
    pub freeze_total_seconds: f64,
    pub freeze_max_seconds: f64,
    pub frozen: bool,
    pub episodes: usize,
    pub cause: String,
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

pub fn summarize(blocks: &[f64], stderr: &[String], cancelled: bool) -> TrialSummary {
    let mut health = StreamHealth::default();
    for line in stderr {
        health.on_stderr(line);
    }
    for block in blocks {
        health.on_block(*block);
    }
    let measured = blocks.len().saturating_sub(GRACE_BLOCKS);
    let mut lost: f64 = health.episodes.iter().filter(|e| e.class == crate::health::Class::Lost).map(|e| e.magnitude).sum();
    let lag = health.lags.back().copied().unwrap_or(0.0);
    let mut freezes: Vec<f64> = health.episodes.iter().map(|e| e.freeze).collect();
    let mut running = health.episodes.len();
    for gap in &health.pending {
        running += 1;
        freezes.push((gap.peak - gap.opening_lag).max(0.0));
        if gap.lowest_after - gap.opening_lag > CATCH_UP_MARGIN {
            lost += (gap.closing_lag - gap.opening_lag).max(0.0);
        }
    }
    if let Some(gap) = health.gap {
        running += 1;
        lost += (lag - gap.opening_lag).max(0.0);
        freezes.push((gap.peak - gap.opening_lag).max(0.0));
    }
    let lags: Vec<f64> = if blocks.len() > GRACE_BLOCKS {
        let anchor = blocks[GRACE_BLOCKS - 1];
        (GRACE_BLOCKS..blocks.len()).map(|k| (k + 1 - GRACE_BLOCKS) as f64 - (blocks[k] - anchor)).collect()
    } else {
        Vec::new()
    };
    let series: Vec<f64> = std::iter::once(0.0).chain(lags).collect();
    TrialSummary {
        ok: measured > 0,
        cancelled,
        blocks: blocks.len(),
        measured_seconds: measured,
        loss_seconds: round2(lost),
        freeze_total_seconds: round2(lost.max(rises(&series, OPENING_THRESHOLD))),
        freeze_max_seconds: round2(freezes.into_iter().fold(0.0, f64::max)),
        frozen: health.frozen,
        episodes: running,
        cause: format!("{:?}", health.cause()).to_lowercase(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    Better,
    NotBetter,
    Impossible,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Verdict {
    pub outcome: Outcome,
    pub reason: String,
    pub nothing_to_compare: bool,
}

#[derive(Debug, Clone, Default)]
pub struct LiveHealth {
    pub state: String,
    pub loss_60s: f64,
    pub restarts: u32,
}

fn rank(state: &str) -> u8 {
    match state {
        "severe" => 2,
        "degraded" => 1,
        _ => 0,
    }
}

impl LiveHealth {
    pub fn worst(&self, other: &LiveHealth) -> LiveHealth {
        let state = if rank(&self.state) >= rank(&other.state) { &self.state } else { &other.state };
        LiveHealth { state: state.clone(), loss_60s: self.loss_60s.max(other.loss_60s), restarts: self.restarts.max(other.restarts) }
    }
}

pub struct Thresholds {
    pub gap_seconds_per_minute: f64,
    pub ratio: f64,
}

impl Default for Thresholds {
    fn default() -> Self {
        Thresholds { gap_seconds_per_minute: 2.0, ratio: 3.0 }
    }
}

fn verdict_of(outcome: Outcome, reason: impl Into<String>, nothing_to_compare: bool) -> Verdict {
    Verdict { outcome, reason: reason.into(), nothing_to_compare }
}

pub fn verdict(live: &LiveHealth, trial: &TrialSummary, thresholds: &Thresholds) -> Verdict {
    if !trial.ok || trial.cancelled {
        return verdict_of(Outcome::Impossible, "the trial could not complete", false);
    }
    if trial.measured_seconds < 20 {
        return verdict_of(Outcome::Impossible, format!("the trial is too short ({} s measured)", trial.measured_seconds), false);
    }
    let lost = trial.freeze_total_seconds.max(trial.loss_seconds);
    let rate = lost * 60.0 / trial.measured_seconds as f64;
    let floor = match live.state.as_str() {
        "severe" => 8.0,
        "degraded" => 2.0,
        _ => 0.0,
    };
    let live_rate = live.loss_60s.max(floor);
    if trial.frozen {
        return verdict_of(Outcome::NotBetter, "the picture froze at the other provider", false);
    }
    if live_rate - rate >= thresholds.gap_seconds_per_minute && rate <= live_rate / thresholds.ratio {
        return verdict_of(Outcome::Better, format!("{rate:.0} s lost per minute at the other provider against {live_rate:.0} s here"), false);
    }
    if live.restarts >= 2 && rate <= 1.0 {
        return verdict_of(Outcome::Better, format!("restarted {} times here, {rate:.0} s lost per minute at the other provider", live.restarts), false);
    }
    if live_rate < 0.5 && rate < 0.5 {
        return verdict_of(Outcome::Impossible, "neither the live stream nor the trial stutters: nothing to compare", true);
    }
    if rate > 1.0 && rate >= live_rate / thresholds.ratio {
        return verdict_of(Outcome::NotBetter, format!("the other provider stutters too ({rate:.0} s per minute against {live_rate:.0} s here)"), false);
    }
    verdict_of(Outcome::NotBetter, format!("the gap is too small ({rate:.0} s against {live_rate:.0} s per minute)"), false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flowing(count: usize) -> Vec<f64> {
        (0..count).map(|i| i as f64).collect()
    }

    fn with_freeze(length: usize, caught_up: bool) -> Vec<f64> {
        let mut trace = flowing(30);
        let last = *trace.last().unwrap();
        trace.extend(std::iter::repeat_n(last, length));
        if caught_up {
            trace.push(last + 2.0 * length as f64 + 1.0);
        }
        trace
    }

    #[test]
    fn a_clean_trial_measures_after_the_grace_and_loses_nothing() {
        let summary = summarize(&flowing(40), &[], false);

        assert!(summary.ok);
        assert_eq!((summary.measured_seconds, summary.loss_seconds, summary.freeze_total_seconds, summary.episodes), (25, 0.0, 0.0, 0));
    }

    #[test]
    fn a_caught_up_freeze_still_counts_as_a_freeze_in_a_trial() {
        let summary = summarize(&with_freeze(6, true), &[], false);

        assert!(summary.loss_seconds == 0.0 || summary.loss_seconds < 1.0, "{summary:?}");
        assert!(summary.freeze_total_seconds >= 5.0, "{summary:?}");
    }

    #[test]
    fn a_gap_still_open_when_the_trial_ends_is_counted_with_the_current_lag() {
        let mut trace = flowing(30);
        trace.extend([29.0; 6]);

        let summary = summarize(&trace, &[], false);

        assert!(summary.loss_seconds >= 4.0 && summary.episodes == 1, "{summary:?}");
    }

    #[test]
    fn an_empty_trial_is_not_ok_and_a_cancelled_one_says_so() {
        let summary = summarize(&flowing(10), &[], true);

        assert!(!summary.ok && summary.cancelled);
    }

    fn bad_trace() -> Vec<f64> {
        with_freeze(8, false).into_iter().chain((1..=40).map(|i| 29.0 + i as f64)).collect()
    }

    fn degraded() -> LiveHealth {
        LiveHealth { state: "degraded".into(), loss_60s: 5.0, ..LiveHealth::default() }
    }

    #[test]
    fn the_backup_is_better_only_with_a_clear_gap_and_ratio() {
        let clean = summarize(&flowing(70), &[], false);
        let thresholds = Thresholds::default();

        assert_eq!(verdict(&degraded(), &clean, &thresholds).outcome, Outcome::Better);
        let also_bad = summarize(&bad_trace(), &[], false);
        assert_eq!(verdict(&degraded(), &also_bad, &thresholds).outcome, Outcome::NotBetter);
        assert_eq!(verdict(&degraded(), &TrialSummary::default(), &thresholds).outcome, Outcome::Impossible);
    }

    #[test]
    fn a_frozen_picture_at_the_other_provider_is_never_better() {
        let mut frozen = summarize(&flowing(70), &[], false);
        frozen.frozen = true;

        assert_eq!(verdict(&degraded(), &frozen, &Thresholds::default()).outcome, Outcome::NotBetter);
    }

    #[test]
    fn repeated_restarts_here_make_a_quiet_trial_better() {
        let clean = summarize(&flowing(70), &[], false);
        let live = LiveHealth { restarts: 2, ..LiveHealth::default() };

        assert_eq!(verdict(&live, &clean, &Thresholds::default()).outcome, Outcome::Better);
    }

    #[test]
    fn two_quiet_streams_teach_nothing_and_a_short_trial_is_impossible() {
        let clean = summarize(&flowing(70), &[], false);
        let quiet = verdict(&LiveHealth::default(), &clean, &Thresholds::default());

        assert!(quiet.outcome == Outcome::Impossible && quiet.nothing_to_compare);
        assert_eq!(verdict(&degraded(), &summarize(&flowing(30), &[], false), &Thresholds::default()).outcome, Outcome::Impossible);
    }

    #[test]
    fn the_worst_of_the_health_at_the_decision_and_at_the_end_is_what_the_verdict_sees() {
        let at_decision = LiveHealth { state: "severe".into(), loss_60s: 12.0, restarts: 0 };
        let at_end = LiveHealth { state: "healthy".into(), loss_60s: 0.0, restarts: 2 };

        let worst = at_decision.worst(&at_end);

        assert_eq!((worst.state.as_str(), worst.loss_60s, worst.restarts), ("severe", 12.0, 2));
        assert_eq!(at_end.worst(&at_end).state, "healthy");
    }
}
