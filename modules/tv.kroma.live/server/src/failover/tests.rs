use super::*;

fn observation(now: f64, provider: ProviderId, media: bool) -> Observation {
    Observation { now, provider: Some(provider), paused: false, media, unstable: false, viewers: 2, program: Some("Match".into()), channel: Some("VIP: CANAL+ RAW".into()) }
}

fn run(policy: &mut FailoverPolicy, from: f64, to: f64, template: &Observation) -> Vec<(f64, Action)> {
    let mut out = Vec::new();
    let mut now = from;
    while now <= to {
        let obs = Observation { now, ..template.clone() };
        if let Some(action) = policy.observe(&obs) {
            policy.acknowledge(action, &obs);
            out.push((now, action));
        }
        now += 10.0;
    }
    out
}

fn policy() -> FailoverPolicy {
    FailoverPolicy::new(true, Limits::default(), Persisted::default(), 1_000_000.0)
}

#[test]
fn a_dead_primary_in_front_of_viewers_fails_over_once_after_forty_five_seconds() {
    let mut p = policy();
    let dead = observation(0.0, ProviderId::Primary, false);

    let acted = run(&mut p, 1000.0, 1100.0, &dead);

    assert_eq!(acted, [(1050.0, Action::Failover)]);
    assert_eq!(run(&mut p, 1060.0, 1100.0, &dead), []);
    assert!(p.primary_is_down() && p.zap_provider() == ProviderId::Backup);
}

#[test]
fn nobody_watching_or_no_backup_means_nothing() {
    let mut idle = policy();
    let mut empty = observation(0.0, ProviderId::Primary, false);
    empty.viewers = 0;
    assert_eq!(run(&mut idle, 1000.0, 1200.0, &empty), []);
    let mut alone = FailoverPolicy::new(false, Limits::default(), Persisted::default(), 1e6);
    assert_eq!(run(&mut alone, 1000.0, 1200.0, &observation(0.0, ProviderId::Primary, false)), []);
}

#[test]
fn a_zap_starting_does_not_read_as_a_dead_stream() {
    let mut p = policy();
    let mut zap = observation(0.0, ProviderId::Primary, false);

    assert_eq!(run(&mut p, 1000.0, 1040.0, &zap), []);
    zap.channel = Some("TF1".into());
    assert_eq!(run(&mut p, 1050.0, 1090.0, &zap), []);
}

#[test]
fn a_stuttering_flow_tries_the_backup_then_proposes_only_if_it_is_better() {
    let mut p = policy();
    let mut bad = observation(0.0, ProviderId::Primary, true);
    bad.unstable = true;

    let first = run(&mut p, 1000.0, 1040.0, &bad);
    assert_eq!(first, [(1020.0, Action::TrialBackup)]);
    let id = 1;
    assert_eq!(run(&mut p, 1030.0, 1200.0, &bad), []);
    assert!(p.trial_done(1100.0, Outcome::Better, "x", false, Some(id)));
    assert_eq!(run(&mut p, 1110.0, 1120.0, &bad), [(1110.0, Action::ProposeTrial)]);
}

#[test]
fn a_trial_that_found_nothing_better_is_reported_once_the_incident_is_settled() {
    let mut p = policy();
    let mut bad = observation(0.0, ProviderId::Primary, true);
    bad.unstable = true;
    run(&mut p, 1000.0, 1020.0, &bad);
    p.trial_done(1080.0, Outcome::NotBetter, "same", false, Some(1));

    let acted = run(&mut p, 1090.0, 1200.0, &bad);

    assert_eq!(acted.iter().map(|a| a.1).collect::<Vec<_>>(), [Action::ReportTrial]);
    assert!(acted[0].0 >= 1000.0 + 120.0);
}

#[test]
fn a_late_trial_result_does_not_overwrite_the_most_recent() {
    let mut p = policy();
    let first = p.did_trial(0.0, ProviderId::Primary, Some("a".into()));
    let second = p.did_trial(10.0, ProviderId::Primary, Some("a".into()));

    assert!(!p.trial_done(20.0, Outcome::Better, "late", false, Some(first)));
    assert!(p.trial_done(21.0, Outcome::NotBetter, "ok", false, Some(second)));
    assert_eq!(p.saved.trial_result.as_ref().unwrap().outcome, Outcome::NotBetter);
}

#[test]
fn on_the_backup_a_finished_programme_proposes_the_return_once() {
    let mut p = policy();
    p.did_failover(900.0);
    p.mark_primary_down(900.0);
    assert!(!p.probe_primary(true));
    assert!(p.probe_primary(true));
    let mut on_backup = observation(1000.0, ProviderId::Backup, true);
    assert_eq!(p.observe(&on_backup), None);
    on_backup.now = 1010.0;
    on_backup.program = Some("News".into());

    assert_eq!(p.observe(&on_backup), Some(Action::SuggestReturn));
    p.did_suggest_return(1010.0, Some("News".into()));
    on_backup.now = 1020.0;
    assert_eq!(p.observe(&on_backup), None);
}

#[test]
fn while_idle_on_the_backup_the_return_to_the_primary_is_retried_every_five_minutes() {
    let mut p = policy();
    let mut idle = observation(0.0, ProviderId::Backup, true);
    idle.paused = true;

    let acted = run(&mut p, 1000.0, 1700.0, &idle);

    assert_eq!(acted.iter().map(|a| a.0).collect::<Vec<_>>(), [1000.0, 1300.0, 1600.0]);
}

#[test]
fn when_the_backup_dies_while_the_primary_is_fine_it_fails_back() {
    let mut p = policy();
    let dead = observation(0.0, ProviderId::Backup, false);

    let acted = run(&mut p, 1000.0, 1100.0, &dead);

    assert_eq!(acted.first(), Some(&(1050.0, Action::Failback)));
}

#[test]
fn both_providers_down_alerts_once_and_recovery_is_announced_when_someone_watches() {
    let mut p = policy();
    p.did_failover(900.0);
    let dead = observation(0.0, ProviderId::Backup, false);

    let acted = run(&mut p, 1000.0, 1060.0, &dead);
    assert_eq!(acted, [(1050.0, Action::BothDown)]);
    assert_eq!(run(&mut p, 1060.0, 1100.0, &dead), []);

    let alive = observation(0.0, ProviderId::Backup, true);
    let back = run(&mut p, 1110.0, 1160.0, &alive);
    assert_eq!(back.first().map(|a| a.1), Some(Action::Recovered));
}

#[test]
fn the_primary_broadcasting_again_clears_its_outage() {
    let mut p = policy();
    p.did_failover(900.0);
    assert!(p.primary_is_down());

    p.observe(&observation(1000.0, ProviderId::Primary, true));

    assert!(!p.primary_is_down());
}

#[test]
fn a_restart_keeps_the_state_and_cancels_a_trial_that_was_running() {
    let mut p = policy();
    p.did_failover(900.0);
    p.did_trial(950.0, ProviderId::Primary, Some("a".into()));
    let json = serde_json::to_string(&p.saved).unwrap();

    let reloaded = FailoverPolicy::new(true, Limits::default(), serde_json::from_str(&json).unwrap(), 1000.0);

    assert_eq!(reloaded.saved.primary_down_since, Some(900.0));
    assert_eq!(reloaded.saved.trial_result.as_ref().map(|r| r.outcome), Some(Outcome::Impossible));
    assert!(serde_json::from_str::<Persisted>("{\"unknown\": 1}").is_ok());
}

#[test]
fn bursts_of_stuttering_add_up_over_the_sliding_window() {
    let mut p = policy();
    let mut bad = observation(0.0, ProviderId::Primary, true);
    let good = observation(0.0, ProviderId::Primary, true);
    bad.unstable = true;

    for burst in 0..3 {
        let start = 1000.0 + burst as f64 * 100.0;
        run(&mut p, start, start + 50.0, &bad);
        run(&mut p, start + 60.0, start + 90.0, &good);
    }

    assert!(p.unstable_for(1290.0) >= 120.0, "{}", p.unstable_for(1290.0));
}

#[test]
fn a_trouble_is_only_declared_after_twenty_seconds_of_silence() {
    let mut p = policy();
    let dead = observation(0.0, ProviderId::Primary, false);
    p.observe(&Observation { now: 1000.0, ..dead.clone() });
    assert!(!p.trouble(1010.0));
    assert!(p.trouble(1021.0));
}
