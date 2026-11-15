use super::*;
use crate::matching::Verdict;

fn row(index: usize, id: &str, number: u32, reading: Option<ScreenReading>, verdict: Option<Verdict>) -> Row {
    Row { index, id: id.into(), name: format!("LIGUE1+ {number}"), number: Some(number), state: RowState::Ok, reason: String::new(), reading, verdict, why: String::new(), target: None, target_why: String::new() }
}

fn live(team1: &str, team2: &str, kind: &str) -> ScreenReading {
    let mut r = ScreenReading::unknown();
    r.kind = kind.into();
    r.team1 = team1.into();
    r.team2 = team2.into();
    r.score = "1-0".into();
    r.confidence = 0.9;
    r
}

fn scan(id: u32, at: i64, rows: Vec<Row>) -> Scan {
    let groups = build_groups(&rows);
    Scan { id, at, provider: "backup".into(), query: "ligue1+".into(), compared: true, search: String::new(), live: Some("p:1".into()), by: "u1".into(), ref_teams: vec!["lens".into(), "paris fc".into()], ref_at: Some(at), rows, groups }
}

#[test]
fn the_same_match_on_two_channels_is_one_group_and_the_best_judged_channel_represents_it() {
    let rows = vec![
        row(0, "s:4", 4, Some(live("Lens", "Paris FC", "direct")), Some(Verdict::Probable)),
        row(1, "s:7", 7, Some(live("Lens", "Paris FC", "direct")), Some(Verdict::Yes)),
        row(2, "s:9", 9, Some(live("Lyon", "Monaco", "direct")), Some(Verdict::No)),
    ];

    let groups = build_groups(&rows);

    assert_eq!(groups.len(), 2);
    assert_eq!((groups[0].n, groups[0].verdict, groups[0].members[0].id.as_str()), (1, Some(Verdict::Yes), "s:7"));
    assert_eq!(groups[0].members.len(), 2);
    assert_eq!(groups[1].members[0].id, "s:9");
}

#[test]
fn a_summary_of_the_same_teams_is_a_group_of_its_own_after_the_live_ones() {
    let rows = vec![row(0, "s:3", 3, Some(live("Lens", "Paris FC", "resume")), None), row(1, "s:5", 5, Some(live("Lens", "Paris FC", "direct")), None)];

    let groups = build_groups(&rows);

    assert_eq!(groups.len(), 2);
    assert_eq!((groups[0].kind.as_str(), groups[1].kind.as_str()), ("direct", "resume"));
}

#[test]
fn black_and_unanswered_channels_form_no_group() {
    let mut black = row(0, "s:1", 1, None, None);
    black.state = RowState::Black;

    assert!(build_groups(&[black]).is_empty());
}

#[test]
fn a_group_reference_resolves_to_its_first_channel_by_scan_and_by_number() {
    let cache = ScanCache::default();
    cache.add(scan(3, 1_000, vec![row(0, "s:7", 7, Some(live("Lens", "Paris FC", "direct")), Some(Verdict::Yes))]));

    assert_eq!(cache.resolve_ref("#3.1", 2_000), Some("s:7".into()));
    assert_eq!(cache.resolve_ref("#1", 2_000), Some("s:7".into()));
    assert_eq!(cache.resolve_ref("#3.2", 2_000), None);
    assert_eq!(cache.resolve_ref("#9.1", 2_000), None);
    assert_eq!(cache.resolve_ref("s:7", 2_000), None);
}

#[test]
fn a_bare_group_number_is_refused_when_two_scans_are_in_memory() {
    let cache = ScanCache::default();
    cache.add(scan(1, 1_000, vec![row(0, "s:7", 7, Some(live("Lens", "Paris FC", "direct")), None)]));
    cache.add(scan(2, 1_500, vec![row(0, "s:8", 8, Some(live("Lyon", "Monaco", "direct")), None)]));

    assert_eq!(cache.resolve_ref("#1", 2_000), None);
    assert_eq!(cache.resolve_ref("#2.1", 2_000), Some("s:8".into()));
}

#[test]
fn a_scan_is_forgotten_after_thirty_minutes_and_only_five_are_kept() {
    let cache = ScanCache::default();
    cache.add(scan(1, 0, vec![row(0, "s:7", 7, Some(live("Lens", "Paris FC", "direct")), None)]));
    for id in 2..=7 {
        cache.add(scan(id, 1_800_000, vec![]));
    }

    assert_eq!(cache.resolve_ref("#1.1", 1_800_001), None);
    assert_eq!(cache.alive(1_800_001).len(), 5);
}

#[test]
fn a_verdict_only_counts_when_the_scan_was_compared_to_the_same_live_channel_recently() {
    let cache = ScanCache::default();
    cache.add(scan(1, 1_000, vec![row(0, "s:7", 7, Some(live("Lens", "Paris FC", "direct")), Some(Verdict::Yes))]));

    let same = cache.judged("s:7", "p:1", 2_000);
    let other_live = cache.judged("s:7", "p:2", 2_000);
    let too_old = cache.judged("s:7", "p:1", 1_000 + VERDICT_MAX_AGE_MS + 1);

    assert_eq!(same.map(|j| j.verdict), Some(Verdict::Yes));
    assert!(other_live.is_none() && too_old.is_none());
}

#[test]
fn the_last_reading_with_teams_is_remembered_per_channel_for_ten_minutes() {
    let seen = LiveSeen::default();
    seen.set("p:1", &live("Lens", "Paris FC", "direct"), 1_000);
    seen.set("p:1", &live("", "", "plateau"), 2_000);

    assert_eq!(seen.teams("p:1", 3_000, LIVE_MEMORY_MS).map(|s| s.at), Some(1_000));
    assert!(seen.teams("p:2", 3_000, LIVE_MEMORY_MS).is_none());
    assert!(seen.teams("p:1", 1_000 + LIVE_MEMORY_MS + 1, LIVE_MEMORY_MS).is_none());
}

#[test]
fn the_state_line_lists_the_groups_of_the_latest_scan_with_their_references() {
    let cache = ScanCache::default();
    cache.add(scan(3, 1_000, vec![row(0, "s:7", 7, Some(live("Lens", "Paris FC", "direct")), Some(Verdict::Yes))]));

    let lines = cache.summary_lines(2_000);

    assert!(lines[0].starts_with("[état] Scan #3") && lines[0].contains("#3.1") && lines[0].contains("s:7"));
}
