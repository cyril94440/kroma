use super::*;

const FOOT: &str = "FR: CANAL+ FOOT RAW";
const VIP: &str = "VIP: CANAL+ FOOT RAW";
const HD: &str = "FR: CANAL+FOOT HD";
const OTHER: &str = "FR: TF1 FHD";

fn family() -> Vec<String> {
    [FOOT, VIP, HD].iter().map(|name| name.to_string()).collect()
}

fn policy() -> FailoverPolicy {
    FailoverPolicy::new(true, Limits::default(), Persisted::default(), 1_000_000.0)
}

fn watch(policy: &mut FailoverPolicy, now: f64, media: bool, channel: &str) -> Option<Action> {
    let observation = Observation { now, provider: Some(ProviderId::Primary), paused: false, media, unstable: false, viewers: 2, program: None, channel: Some(channel.into()) };
    policy.observe(&observation)
}

fn silence_until_decision(policy: &mut FailoverPolicy, from: f64, channel: &str) -> Option<(f64, Action)> {
    (0..7).find_map(|step| {
        let now = from + f64::from(step) * 10.0;
        watch(policy, now, false, channel).map(|action| (now, action))
    })
}

#[test]
fn a_family_gets_two_variant_switches_per_window_and_never_the_same_variant_twice() {
    let mut p = policy();
    p.did_variant_switch(1000.0, FOOT, VIP, &family());
    p.did_variant_switch(1060.0, VIP, HD, &family());

    let (left, muted) = p.variant_state(1100.0, HD, &family());

    assert_eq!(left, 0);
    assert!(muted.contains(FOOT) && muted.contains(VIP));
    assert!(p.variants_tried());
}

#[test]
fn a_muted_variant_is_a_candidate_again_after_ten_minutes_and_the_budget_comes_back() {
    let mut p = policy();
    p.did_variant_switch(1000.0, FOOT, VIP, &family());
    p.did_variant_switch(1060.0, VIP, HD, &family());

    let (left, muted) = p.variant_state(1000.0 + AUTO_COOLDOWN + 70.0, HD, &family());

    assert_eq!(left, 2);
    assert!(!muted.contains(FOOT));
}

#[test]
fn the_message_about_a_cut_family_is_claimed_once_per_ten_minutes() {
    let mut p = policy();
    p.did_channel_down(1000.0, Some(FOOT), Some(&family()));

    assert!(p.claim_family_alert(1001.0));
    assert!(!p.claim_family_alert(1100.0));
    assert!(p.claim_family_alert(1001.0 + AUTO_COOLDOWN));
}

#[test]
fn a_muted_channel_does_not_fail_over_again_before_ten_minutes_but_another_one_does() {
    let mut p = policy();
    p.did_channel_down(1000.0, Some("FR: TF1 FHD"), None);

    assert_eq!(silence_until_decision(&mut p, 1010.0, "FR: TF1 FHD"), None);
    p.dead_since = None;
    assert_eq!(silence_until_decision(&mut p, 1100.0, "FR: M6 HD").map(|d| d.1), Some(Action::Failover));
    p.dead_since = None;
    assert_eq!(silence_until_decision(&mut p, 1000.0 + AUTO_COOLDOWN + 10.0, "FR: TF1 FHD").map(|d| d.1), Some(Action::Failover));
}

#[test]
fn a_channel_cut_alone_neither_declares_the_provider_down_nor_arms_the_cooldown() {
    let mut p = policy();

    p.did_channel_down(1000.0, Some("TF1"), None);

    assert!(!p.primary_is_down());
    assert_eq!(p.zap_provider(), ProviderId::Primary);
    assert_eq!(p.saved.last_auto_at, None);
}

#[test]
fn a_manual_zap_forgets_the_family_and_so_does_a_steady_stream_for_two_minutes() {
    let mut p = policy();
    p.did_variant_switch(1000.0, FOOT, VIP, &family());
    watch(&mut p, 1010.0, true, VIP);
    assert!(p.saved.variant_family.is_some());

    watch(&mut p, 1020.0, true, OTHER);

    assert!(p.saved.variant_family.is_none());
    let mut steady = policy();
    steady.did_variant_switch(1000.0, FOOT, VIP, &family());
    for step in 0..14 {
        watch(&mut steady, 1010.0 + f64::from(step) * 10.0, true, VIP);
    }
    assert!(steady.saved.variant_family.is_none());
}

#[test]
fn a_variant_taken_by_the_switch_has_its_own_forty_five_seconds_before_being_judged_mute() {
    let mut p = policy();
    for step in 0..6 {
        watch(&mut p, 100.0 + f64::from(step) * 10.0, false, FOOT);
    }
    assert_eq!(p.dead_since(), Some(100.0));

    assert_eq!(watch(&mut p, 160.0, false, VIP), None);
    assert_eq!(watch(&mut p, 170.0, false, VIP), None);
    assert_eq!(p.dead_since(), Some(160.0));
    assert_eq!(watch(&mut p, 180.0, true, VIP), None);
    assert_eq!(p.dead_since(), None);
}

#[test]
fn a_general_outage_without_a_remembered_family_fails_over_after_forty_five_seconds() {
    let mut p = policy();
    p.did_channel_down(1000.0, Some(FOOT), Some(&family()));

    let decision = silence_until_decision(&mut p, 1100.0, OTHER);

    assert_eq!(decision.map(|d| d.1), Some(Action::Failover));
}

#[test]
fn media_that_returned_during_the_probes_is_seen_as_such() {
    let mut p = policy();
    watch(&mut p, 100.0, false, FOOT);
    let began = p.dead_since();
    assert!(!p.media_returned(began));

    watch(&mut p, 110.0, true, FOOT);

    assert!(p.media_returned(began));
    assert!(!p.media_returned(None));
}

#[test]
fn an_unreadable_field_is_ignored_alone_and_the_rest_of_the_state_is_kept() {
    let text = r#"{"primaryDownSince": 9000.0, "lastAutoAt": "yesterday", "bothDownAlerted": "yes", "channelDown": {"channel": "TF1", "at": 9500.0}}"#;
    let (loaded, unreadable) = Persisted::from_json(text);

    let p = FailoverPolicy::new(true, Limits::default(), loaded, 10_000.0);

    assert_eq!(p.saved.primary_down_since, Some(9000.0));
    assert_eq!(p.saved.channel_down, Some(ChannelDown { channel: Some("TF1".into()), at: 9500.0 }));
    assert_eq!(unreadable, ["lastAutoAt", "bothDownAlerted"]);
    assert_eq!(Persisted::from_json("not json").1, ["unreadable"]);
}

#[test]
fn stamps_from_the_future_and_oversized_values_are_dropped_with_their_names() {
    let now = 10_000.0;
    let saved = Persisted {
        primary_down_since: Some(now + 86_400.0),
        last_suggest_at: Some(f64::NAN),
        channel_at_switch: Some("x".repeat(301)),
        channel_down: Some(ChannelDown { channel: Some("TF1".into()), at: now + 9999.0 }),
        variant_family: Some(Family { switches: vec![1e18], ..Family::default() }),
        last_trial_at: Some(-5.0),
        ..Persisted::default()
    };

    let p = FailoverPolicy::new(true, Limits::default(), saved, now);

    assert_eq!(p.saved.primary_down_since, None);
    assert_eq!(p.zap_provider(), ProviderId::Primary);
    assert!(p.saved.channel_down.is_none() && p.saved.variant_family.is_none() && p.saved.last_trial_at.is_none());
    let mut names = p.ignored_state.clone();
    names.sort_unstable();
    assert_eq!(names, ["channelAtSwitch", "channelDown", "lastSuggestAt", "lastTrialAt", "primaryDownSince", "variantFamily"]);
}

#[test]
fn a_family_survives_a_restart_in_the_middle_of_an_incident() {
    let mut p = policy();
    p.did_variant_switch(1_000_000.0, FOOT, VIP, &family());
    p.did_variant_switch(1_000_060.0, VIP, HD, &family());
    let json = serde_json::to_string(&p.saved).unwrap();

    let (loaded, unreadable) = Persisted::from_json(&json);
    let reborn = FailoverPolicy::new(true, Limits::default(), loaded, 1_000_400.0);

    assert!(unreadable.is_empty() && reborn.ignored_state.is_empty());
    assert_eq!(reborn.variant_state(1_000_400.0, HD, &family()).0, 0);
}

#[test]
fn an_older_state_without_the_new_keys_loads_without_ignoring_anything() {
    let (loaded, unreadable) = Persisted::from_json(r#"{"lastAutoAt": 900.0}"#);

    let p = FailoverPolicy::new(true, Limits::default(), loaded, 1000.0);

    assert!(unreadable.is_empty() && p.ignored_state.is_empty() && p.saved.variant_family.is_none());
}

#[test]
fn undoing_a_dropped_action_restores_what_the_observation_had_already_spent() {
    let mut p = policy();
    p.did_failover(900.0);
    let dead = Observation { now: 1000.0, provider: Some(ProviderId::Backup), paused: false, media: false, unstable: false, viewers: 2, program: None, channel: Some("a".into()) };
    for step in 0..6 {
        p.observe(&Observation { now: 1000.0 + f64::from(step) * 10.0, ..dead.clone() });
    }
    let alive = Observation { media: true, ..dead.clone() };
    p.saved.both_down_alerted = true;
    let mut action = None;
    for step in 0..4 {
        action = action.or(p.observe(&Observation { now: 1100.0 + f64::from(step) * 10.0, ..alive.clone() }));
    }
    assert_eq!(action, Some(Action::Recovered));

    p.undo(Action::Recovered);

    assert!(p.saved.both_down_alerted);
}
