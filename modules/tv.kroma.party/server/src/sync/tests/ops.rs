use super::*;

#[test]
fn a_kick_is_decided_on_the_journal_with_a_cooldown() {
    let mut g = SyncGroup::new(0);
    bat(&mut g, "Mamie", 2.0, T0);
    assert_eq!(g.kick("inconnu", true, "", T0).0, KickOutcome::Refused("notInParty"));
    g.log(T0, json!({ "event": "decrochage", "userId": "mamie" }));
    let (out, says) = g.kick("mamie", true, "he annoys me", T0);
    assert_eq!(out, KickOutcome::Kicked { disruptions: 1, reason_stated: true });
    assert_eq!(says[0].key, "party.sync.removedDisruptions");
    assert!(!g.contains("mamie"));
    bat(&mut g, "Mamie", 2.0, T0 + 1000);
    assert_eq!(g.kick("mamie", true, "", T0 + 1000).0, KickOutcome::Refused("recentlyKicked"));
    assert!(matches!(g.kick("mamie", true, "", T0 + 200_000).0, KickOutcome::Kicked { .. }));
}

#[test]
fn a_disconnected_member_is_not_kicked_and_nothing_is_announced() {
    let mut g = SyncGroup::new(0);
    bat(&mut g, "Sebi", 9.0, T0);
    let (out, says) = g.kick("sebi", false, "", T0);
    assert_eq!(out, KickOutcome::Refused("disconnected"));
    assert!(says.is_empty());
    assert!(g.contains("sebi"));
}

#[test]
fn a_kick_without_disruption_says_it_was_requested() {
    let mut g = SyncGroup::new(0);
    bat(&mut g, "Mamie", 2.0, T0);
    let (out, says) = g.kick("mamie", true, "", T0);
    assert_eq!(out, KickOutcome::Kicked { disruptions: 0, reason_stated: false });
    assert_eq!(says[0].key, "party.sync.removedRequested");
}

#[test]
fn go_live_resets_the_target_and_fine_sync_meets_behind_the_slowest() {
    let mut g = SyncGroup::new(0);
    assert_eq!(g.go_live("agent", T0), Err("noActiveParty"));
    bat(&mut g, "Mamie", 3.0, T0);
    bat(&mut g, "Papi", 5.0, T0);
    assert!(g.fine_sync("agent", T0).is_ok());
    assert!((g.target() - 5.15).abs() < 1e-9);
    assert!(g.freeze_until > T0);
    assert!(g.fine_done(&who("Mamie"), FineDone { ms: 40, seeks: 1, ..FineDone::default() }, T0).is_empty());
    let says = g.fine_done(&who("Papi"), FineDone { ms: 60, seeks: 2, ..FineDone::default() }, T0);
    assert_eq!(says[0].key, "party.sync.fineDone");
    assert_eq!(says[0].params["items"].as_array().unwrap().len(), 2);
    assert!(g.go_live("agent", T0).is_ok());
    assert_eq!(g.target(), 0.0);
}

#[test]
fn an_abandoned_campaign_reports_no_fictional_gap() {
    let mut g = SyncGroup::new(0);
    bat(&mut g, "Mamie", 3.0, T0);
    bat(&mut g, "Papi", 5.0, T0);
    g.fine_sync("agent", T0).unwrap();
    g.fine_done(&who("Mamie"), FineDone { ms: 40, seeks: 1, ..FineDone::default() }, T0);
    let says = g.fine_done(&who("Papi"), FineDone { abandoned: true, ..FineDone::default() }, T0);
    let items = says[0].params["items"].as_array().unwrap();
    let papi = items.iter().find(|i| i["name"] == "Papi").unwrap();
    assert_eq!(papi["abandoned"], true);
    assert!(papi.get("ms").is_none());
}

#[test]
fn a_campaign_nobody_finishes_reports_after_the_timeout() {
    let mut g = SyncGroup::new(0);
    bat(&mut g, "Mamie", 3.0, T0);
    bat(&mut g, "Papi", 5.0, T0);
    g.fine_sync("agent", T0).unwrap();
    g.fine_done(&who("Mamie"), FineDone { ms: 40, seeks: 1, ..FineDone::default() }, T0);
    assert!(g.tick(T0 + 10_000).says.is_empty());
    assert_eq!(g.tick(T0 + FINE_REPORT_TIMEOUT_MS + 1000).says.len(), 1);
}

#[test]
fn a_solo_alignment_at_entry_is_announced_honestly() {
    let mut g = SyncGroup::new(0);
    bat(&mut g, "Mamie", 3.0, T0);
    let good = g.fine_done(&who("Mamie"), FineDone { ms: 90, solo: true, ..FineDone::default() }, T0);
    assert_eq!(good[0].key, "party.sync.fineSolo");
    let bad = g.fine_done(&who("Mamie"), FineDone { ms: 2400, solo: true, ..FineDone::default() }, T0);
    assert_eq!(bad[0].key, "party.sync.fineSoloMissed");
    assert_eq!(bad[0].params["seconds"], "2.4");
}

