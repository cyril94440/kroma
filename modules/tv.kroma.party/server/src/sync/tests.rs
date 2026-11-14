mod ops;

use super::*;

pub(super) const T0: i64 = 5_000_000;

pub(super) fn who(name: &str) -> Person {
    Person::new(&name.to_lowercase(), name)
}

pub(super) fn bat(g: &mut SyncGroup, name: &str, delay: f64, now: i64) -> Vec<Say> {
    g.beat(&who(name), &Beat { enabled: true, delay, ..Beat::default() }, now)
}

pub(super) fn bat_with(g: &mut SyncGroup, name: &str, delay: f64, now: i64, f: impl Fn(&mut Beat)) {
    let mut b = Beat { enabled: true, delay, ..Beat::default() };
    f(&mut b);
    g.beat(&who(name), &b, now);
}

pub(super) fn events(g: &SyncGroup, name: &str) -> usize {
    g.journal.iter().filter(|e| e["event"] == name).count()
}

pub(super) fn state(g: &mut SyncGroup, now: i64) -> SyncState {
    g.tick(now).state.expect("a state is broadcast")
}

#[test]
fn the_first_alignment_is_capped_and_the_target_never_ratchets() {
    let mut g = SyncGroup::new(0);
    bat(&mut g, "Mamie", 3.0, T0);
    bat(&mut g, "Papi", 20.0, T0);
    let s = state(&mut g, T0);
    assert_eq!(s.target, 12.0);
    assert_eq!(s.by.unwrap().name, "Papi");
    assert!(s.capped);
    assert_eq!(events(&g, "calage_initial"), 1);

    bat(&mut g, "Papi", 4.0, T0);
    bat(&mut g, "Mamie", 4.0, T0);
    let s = state(&mut g, T0);
    assert!((3.5..=4.5).contains(&s.target), "the target joins the group: {}", s.target);

    bat(&mut g, "Papi", 4.3, T0);
    bat(&mut g, "Mamie", 4.0, T0);
    assert!(state(&mut g, T0).target <= 4.8);
}

#[test]
fn a_plain_lag_does_not_pull_the_group() {
    let mut g = SyncGroup::new(0);
    bat(&mut g, "Mamie", 5.0, T0);
    bat(&mut g, "Papi", 5.0, T0);
    g.tick(T0);
    bat(&mut g, "Mamie", 5.0, T0);
    bat(&mut g, "Papi", 9.0, T0);
    let before = g.target();
    g.tick(T0);
    assert!(g.target() <= before);
    assert!(g.journal.iter().any(|e| e["event"] == "retard_en_rattrapage" && e["name"] == "Papi"));
}

#[test]
fn the_end_of_a_party_broadcasts_one_empty_state() {
    let mut g = SyncGroup::new(0);
    bat(&mut g, "Mamie", 2.0, T0);
    g.tick(T0);
    g.remove("mamie", T0);
    let first = g.tick(T0 + 2000).state;
    let second = g.tick(T0 + 4000).state;
    assert!(first.is_some_and(|s| s.members.is_empty()));
    assert!(second.is_none());
    assert_eq!(events(&g, "quitte"), 1);
}

#[test]
fn a_join_is_announced_once_per_two_minutes() {
    let mut g = SyncGroup::new(0);
    let first = bat(&mut g, "Mamie", 2.0, T0);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].key, "party.sync.started");
    assert!(bat(&mut g, "Mamie", 2.0, T0).is_empty());
    let second = bat(&mut g, "Papi", 2.0, T0);
    assert_eq!(second[0].key, "party.sync.joined");
}

#[test]
fn a_restart_grace_hides_the_announcements_of_a_party_that_already_existed() {
    let mut g = SyncGroup::new(T0 + 60_000);
    assert!(bat(&mut g, "Mamie", 3.0, T0).is_empty());
    assert!(bat(&mut g, "Papi", 4.0, T0).is_empty());
    assert_eq!(g.member_count(), 2);
}

#[test]
fn a_common_hiccup_moves_the_target_not_the_viewers() {
    let mut g = SyncGroup::new(0);
    bat(&mut g, "A", 6.0, T0);
    bat(&mut g, "B", 6.2, T0);
    g.tick(T0);
    for i in 1..=30 {
        let now = T0 + i * 2000;
        let jump = if now >= T0 + 40_000 { 5.0 } else { 0.0 };
        bat(&mut g, "A", 6.0 + jump, now);
        bat(&mut g, "B", 6.2 + jump, now);
        g.tick(now);
    }
    assert_eq!(events(&g, "retard_en_rattrapage"), 0);
    assert_eq!(events(&g, "decrochage"), 0);
    assert!(events(&g, "groupe_decale") >= 1);
    assert!(g.target() >= 10.0);
}

#[test]
fn a_common_return_towards_live_sends_nobody_backwards() {
    let mut g = SyncGroup::new(0);
    bat(&mut g, "A", 11.0, T0);
    bat(&mut g, "B", 11.0, T0);
    g.tick(T0);
    for i in 1..=15 {
        let now = T0 + i * 2000;
        let (a, b) = if i >= 5 { (6.0, 6.3) } else { (11.0, 11.0) };
        bat(&mut g, "A", a, now);
        bat(&mut g, "B", b, now);
        g.tick(now);
    }
    assert!(g.target() <= 6.9, "the target joins the group: {}", g.target());
}

#[test]
fn restarting_players_keep_the_party_alive_and_leave_after_forty_five_seconds() {
    let mut g = SyncGroup::new(0);
    bat(&mut g, "A", 6.0, T0);
    bat(&mut g, "B", 6.0, T0);
    g.tick(T0);
    let mut now = T0 + 30_000;
    bat(&mut g, "A", 6.0, now);
    let s = state(&mut g, now);
    assert!(g.contains("b"));
    assert_eq!(s.members.len(), 2);
    now += 20_000;
    bat(&mut g, "A", 6.0, now);
    g.tick(now);
    assert!(!g.contains("b"));
}

#[test]
fn burst_restarts_never_dissolve_the_party() {
    let mut g = SyncGroup::new(0);
    bat(&mut g, "Sebi", 6.0, T0);
    bat(&mut g, "Cyril", 6.0, T0);
    g.tick(T0);
    let mut announced = 0;
    for i in 0..15 {
        let start = T0 + 30_000 + i * 20_000;
        for step in 0..10 {
            let now = start + step * 2000;
            if step < 5 {
                announced += bat(&mut g, "Sebi", 6.0 + (i % 2) as f64 * 0.5, now).len();
                announced += bat(&mut g, "Cyril", 6.0, now).len();
            }
            g.tick(now);
        }
    }
    assert_eq!(events(&g, "quitte"), 0);
    assert_eq!(announced, 0);
}

#[test]
fn a_client_recalibration_is_journaled_apart_validated_and_rate_limited() {
    let mut g = SyncGroup::new(0);
    bat(&mut g, "Sebi", 9.0, T0);
    g.client_recalibration(&who("Sebi"), Recal { gap: -4.24, target: 6.0, arrival: false, blocked: true }, T0);
    let e = &g.recalibrations[0];
    assert_eq!(e["gapS"], -4.2);
    assert_eq!(e["blocked"], true);
    assert_eq!(e["direction"], "catchUp");
    assert_eq!(events(&g, "recalage"), 0);
    g.client_recalibration(&who("Sebi"), Recal { gap: 3.0, target: 0.0, arrival: false, blocked: false }, T0);
    g.client_recalibration(&who("Intrus"), Recal { gap: 3.0, target: 0.0, arrival: false, blocked: false }, T0);
    g.client_recalibration(
        &who("Sebi"),
        Recal { gap: f64::INFINITY, target: f64::NAN, arrival: false, blocked: false },
        T0 + 6000,
    );
    assert_eq!(g.recalibrations.len(), 2);
    assert_eq!(g.recalibrations[1]["gapS"], 0.0);
    assert_eq!(g.recalibrations[1]["targetS"], 0.0);
}

#[test]
fn two_pausers_one_resumes_the_group_comes_to_them_capped() {
    let mut g = SyncGroup::new(0);
    let mut now = T0;
    bat(&mut g, "A", 6.3, now);
    bat(&mut g, "B", 6.3, now);
    g.tick(now);
    for i in 0..15 {
        now += 2000;
        let d = 6.3 + f64::from(i) * 2.0;
        bat_with(&mut g, "A", d, now, |b| b.paused = true);
        bat_with(&mut g, "B", d, now, |b| b.paused = true);
        g.tick(now);
    }
    now += 2000;
    bat(&mut g, "A", 19.5, now);
    bat_with(&mut g, "B", 36.0, now, |b| b.paused = true);
    let s = state(&mut g, now);
    assert_eq!(s.target, 12.0);
    assert_eq!(s.by.unwrap().name, "A");
    assert!(g.journal.iter().any(|e| e["event"] == "decrochage" && e["cause"] == "resumeAfterPause"));
    assert_eq!(events(&g, "groupe_decale"), 0);
}

#[test]
fn a_laggard_stuck_at_the_edge_of_its_window_makes_the_group_come_to_it() {
    let mut g = SyncGroup::new(0);
    bat(&mut g, "Cyril", 6.0, T0);
    bat(&mut g, "Sebi", 6.0, T0);
    g.tick(T0);
    for i in 1..=12 {
        let now = T0 + i * 2000;
        bat(&mut g, "Cyril", 6.0, now);
        bat_with(&mut g, "Sebi", 10.5, now, |b| b.pinned = true);
        g.tick(now);
    }
    assert!(events(&g, "plancher_releve") >= 1);
    assert!(g.target() >= 10.0);
}

#[test]
fn a_floor_raise_beyond_the_cap_has_no_effect_and_is_not_journaled() {
    let mut g = SyncGroup::new(0);
    bat(&mut g, "A", 6.0, T0);
    bat(&mut g, "L", 18.0, T0);
    g.tick(T0);
    for i in 1..=30 {
        let now = T0 + i * 2000;
        bat(&mut g, "A", 6.0, now);
        bat_with(&mut g, "L", 18.0, now, |b| b.pinned = true);
        g.tick(now);
    }
    assert_eq!(events(&g, "plancher_releve"), 0);
}

#[test]
fn fine_sync_and_the_active_count_ignore_phantoms() {
    let mut g = SyncGroup::new(0);
    bat(&mut g, "A", 6.0, T0);
    bat(&mut g, "B", 6.0, T0);
    assert_eq!(state(&mut g, T0).active, 2);
    let now = T0 + 20_000;
    bat(&mut g, "A", 6.0, now);
    let s = state(&mut g, now);
    assert_eq!((s.active, s.members.len()), (1, 2));
    assert_eq!(g.fine_sync("agent", now), Err("needTwoMembers"));
}

#[test]
fn a_go_live_is_not_undone_by_latencies_measured_before_the_jump() {
    let mut g = SyncGroup::new(0);
    bat(&mut g, "A", 10.0, T0);
    bat(&mut g, "B", 9.0, T0);
    g.tick(T0);
    assert_eq!(g.target(), 10.0);
    assert!(g.go_live("agent", T0 + 1000).is_ok());
    assert_eq!(g.target(), 0.0);
    g.tick(T0 + 2000);
    assert_eq!(g.target(), 0.0);
    bat(&mut g, "A", 10.0, T0 + 3000);
    g.tick(T0 + 3000);
    assert_eq!(g.target(), 0.0);
    let later = T0 + 6000;
    bat(&mut g, "A", 5.5, later);
    bat(&mut g, "B", 5.0, later);
    g.tick(later);
    assert_eq!(g.target(), 5.5);
}

#[test]
fn the_state_pins_the_json_the_clients_read() {
    let mut g = SyncGroup::new(0);
    bat(&mut g, "Mamie", 3.0, T0);
    bat(&mut g, "Papi", 20.0, T0);
    let json = serde_json::to_value(state(&mut g, T0)).unwrap();
    assert_eq!(
        json,
        json!({
            "target": 12.0,
            "by": { "userId": "papi", "name": "Papi" },
            "capped": true,
            "members": [{ "userId": "mamie", "name": "Mamie" }, { "userId": "papi", "name": "Papi" }],
            "active": 2
        })
    );
}
