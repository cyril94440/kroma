use crate::health::{Cause, Class, State, StreamHealth, DEGRADED_LOSS, GRACE_BLOCKS, SEVERE_LOSS};

fn replay(health: &mut StreamHealth, trace: &[f64]) {
    for time in trace {
        health.on_block(*time);
    }
}

fn healthy(count: usize) -> Vec<f64> {
    (0..count).map(|i| i as f64).collect()
}

fn freeze(mut trace: Vec<f64>, length: usize, caught_up: bool) -> Vec<f64> {
    let last = *trace.last().unwrap();
    trace.extend(std::iter::repeat_n(last, length));
    if caught_up {
        trace.push(last + 2.0 * length as f64 + 1.0);
    }
    trace
}

fn then_flowing(mut trace: Vec<f64>, count: usize) -> Vec<f64> {
    let last = *trace.last().unwrap();
    trace.extend((1..=count).map(|i| last + i as f64));
    trace
}

#[test]
fn the_start_grace_ignores_even_an_absurd_burst_and_the_anchor_lands_on_the_grace_block() {
    let mut health = StreamHealth::default();

    replay(&mut health, &[0.0, 0.0, 0.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 18.0, 19.0]);
    assert!(health.state == State::Starting && !health.is_armed() && health.lags.is_empty());
    health.on_block(20.0);
    assert!(health.is_armed() && health.state == State::Healthy);
    health.on_block(21.0);
    assert_eq!(health.current_lag(), Some(0.0));
}

#[test]
fn a_noisy_healthy_stream_stays_healthy() {
    let mut health = StreamHealth::default();
    let noise: Vec<f64> = (0..120).map(|i| i as f64 + if i % 2 == 1 { 0.2 } else { -0.2 }).collect();

    replay(&mut health, &noise);

    assert!(health.state == State::Healthy && health.episodes_total == 0 && health.channel_loss == 0.0 && !health.frozen);
    assert!(health.gap.is_none() && health.pending.is_empty());
}

#[test]
fn a_caught_up_freeze_is_kept_quiet() {
    let mut health = StreamHealth::default();

    replay(&mut health, &then_flowing(freeze(healthy(30), 3, true), 40));

    assert_eq!((health.episodes_total, health.caught_up_gaps()), (1, 1));
    assert!(health.episodes[0].class == Class::CaughtUp && health.episodes[0].magnitude == 0.0);
    assert!(health.channel_loss == 0.0 && health.recent_loss() == 0.0 && health.state == State::Healthy);
}

#[test]
fn a_lost_freeze_of_three_seconds_degrades_with_its_magnitude() {
    let mut health = StreamHealth::default();

    replay(&mut health, &then_flowing(freeze(healthy(30), 3, false), 40));

    let episode = health.episodes[0];
    assert_eq!(health.episodes_total, 1);
    assert!(episode.class == Class::Lost && (2.0..=3.5).contains(&episode.magnitude), "{episode:?}");
    assert_eq!(health.channel_loss, episode.magnitude);
    assert!((DEGRADED_LOSS..SEVERE_LOSS).contains(&health.recent_loss()) && health.state == State::Degraded);
}

#[test]
fn a_lost_freeze_of_eight_seconds_is_severe() {
    let mut health = StreamHealth::default();

    replay(&mut health, &then_flowing(freeze(healthy(30), 8, false), 40));

    let episode = health.episodes[0];
    assert!(episode.class == Class::Lost && (7.0..=8.5).contains(&episode.magnitude), "{episode:?}");
    assert!(health.channel_loss == episode.magnitude && health.state == State::Severe);
}

#[test]
fn a_closed_gap_does_not_reopen_as_a_phantom_on_its_own_ramp() {
    let mut health = StreamHealth::default();

    replay(&mut health, &then_flowing(freeze(healthy(30), 6, false), 59));

    assert_eq!(health.episodes_total, 1);
    assert!(health.gap.is_none() && health.pending.is_empty());
}

#[test]
fn a_freeze_in_progress_raises_the_frozen_flag_then_lowers_it() {
    let mut health = StreamHealth::default();
    replay(&mut health, &healthy(30));
    assert!(!health.frozen);

    replay(&mut health, &[29.0; 4]);
    assert!(health.frozen);
    replay(&mut health, &(1..8).map(|i| 29.0 + i as f64).collect::<Vec<_>>());

    assert!(!health.frozen);
}

#[test]
fn a_long_freeze_is_severe_before_it_is_classified() {
    let mut health = StreamHealth::default();

    replay(&mut health, &freeze(healthy(30), 14, false));

    assert_eq!(health.state, State::Severe);
}

#[test]
fn going_back_to_healthy_takes_a_lasting_calm() {
    let mut health = StreamHealth::default();
    let trace = freeze(healthy(30), 8, false);
    replay(&mut health, &trace);
    let mut last = *trace.last().unwrap();

    replay(&mut health, &(1..25).map(|i| last + i as f64).collect::<Vec<_>>());
    assert_eq!(health.state, State::Severe);
    last += 24.0;
    replay(&mut health, &(1..200).map(|i| last + i as f64).collect::<Vec<_>>());

    assert_eq!(health.state, State::Healthy);
    assert!(health.channel_loss > 0.0);
}

#[test]
fn a_restart_keeps_the_history_a_pause_forgets_the_blocks_and_a_zap_forgets_all() {
    let mut health = StreamHealth::default();
    let trace = then_flowing(freeze(healthy(30), 6, false), 59);
    replay(&mut health, &trace);
    health.on_stderr("Will reconnect at 1234");
    health.on_stderr("DTS discontinuity");
    assert_eq!((health.source_hints, health.local_hints, health.cause()), (1, 1, Cause::Undetermined));
    let (loss, episodes) = (health.channel_loss, health.episodes_total);
    assert!(loss > 0.0 && episodes == 1);

    health.on_pause();
    assert!(health.state == State::Undetermined && !health.is_armed() && health.blocks() == 0);
    health.on_start();
    assert!(health.state == State::Starting && health.channel_loss == loss && health.episodes.len() == 1);
    health.on_channel_change();

    assert!(health.channel_loss == 0.0 && health.episodes.is_empty() && health.source_hints == 0);
}

#[test]
fn the_cause_is_only_named_on_positive_evidence() {
    let mut health = StreamHealth::default();
    assert_eq!(health.cause(), Cause::Undetermined);
    health.on_stderr("Server returned 503 Service Unavailable");
    assert_eq!(health.cause(), Cause::Source);
    health.on_stderr("Non-monotonous DTS in output stream");
    assert_eq!(health.cause(), Cause::Undetermined);
    let mut other = StreamHealth::default();
    other.on_stderr("non monoton");
    assert_eq!(other.cause(), Cause::Local);
}

fn from_lags(lags: &[f64]) -> Vec<f64> {
    let base = GRACE_BLOCKS as f64;
    let mut trace: Vec<f64> = (0..GRACE_BLOCKS).map(|i| i as f64).collect();
    trace.extend(std::iter::once(0.0).chain(lags.iter().copied()).enumerate().map(|(at, lag)| base + at as f64 - lag));
    trace
}

#[test]
fn a_caught_up_gap_then_a_real_one_inside_the_classification_delay_counts_three_seconds_not_six() {
    let rise = [1.0, 2.0, 3.0, 3.0, 3.0, 3.0];
    let mut lags = vec![0.0; 10];
    lags.extend(rise);
    lags.extend([2.0, 0.0]);
    lags.extend([0.0; 6]);
    lags.extend(rise);
    lags.extend([3.0; 40]);
    let mut health = StreamHealth::default();

    replay(&mut health, &from_lags(&lags));

    assert_eq!(health.channel_loss, 3.0);
    assert_eq!(health.caught_up_gaps(), 1);
    assert_eq!(health.episodes.iter().map(|e| e.class).collect::<Vec<_>>(), [Class::CaughtUp, Class::Lost]);
}
