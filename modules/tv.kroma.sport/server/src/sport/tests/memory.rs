use super::support::{event, live_match, reading, scan_row, NOW};
use crate::sport::{cadence_seconds, need_scan, Certainty, Memory, State};

#[test]
fn a_seen_channel_holds_until_the_match_ends_and_survives_a_restart() {
    let mut memory = Memory::default();
    let match_ = live_match();
    memory.note_row(
        &scan_row(
            "p:11",
            "ok",
            Some(reading("Lens", "Lille", "direct", 0.9)),
            1,
        ),
        std::slice::from_ref(&match_),
        NOW,
    );

    memory.expire(std::slice::from_ref(&match_), NOW + 3000.0);
    let restored: Memory = serde_json::from_str(&serde_json::to_string(&memory).unwrap()).unwrap();
    let mut finished = match_.clone();
    finished.state = State::Post;
    let mut after = restored.clone();
    after.expire(&[finished], NOW + 6000.0);

    assert_eq!(
        memory.seen_channel("foot:1").and_then(|s| s.certainty),
        Some(Certainty::Seen)
    );
    assert!(restored.seen_channel("foot:1").is_some());
    assert!(after.seen_channel("foot:1").is_none());
}

#[test]
fn a_black_screen_is_forgotten_at_the_next_kickoff() {
    let mut memory = Memory::default();
    memory.note_row(&scan_row("p:12", "noir", None, 2), &[], NOW);

    memory.expire(
        &[event("foot:1", "A", "B", State::Pre, NOW + 600.0)],
        NOW + 60.0,
    );
    let kept = !memory.image.is_empty();
    memory.expire(
        &[event("foot:1", "A", "B", State::In, NOW + 600.0)],
        NOW + 700.0,
    );

    assert!(kept);
    assert!(memory.image.is_empty());
}

#[test]
fn a_failed_capture_does_not_downgrade_a_seen_channel() {
    let mut memory = Memory::default();
    let match_ = live_match();
    memory.note_row(
        &scan_row(
            "p:11",
            "ok",
            Some(reading("Lens", "Lille", "direct", 0.9)),
            1,
        ),
        std::slice::from_ref(&match_),
        NOW,
    );

    let changed = memory.note_row(
        &scan_row("p:11", "erreur", None, 1),
        std::slice::from_ref(&match_),
        NOW + 5.0,
    );

    assert!(!changed);
    assert!(memory.seen_channel("foot:1").is_some());
}

#[test]
fn no_scan_while_everything_is_identified_except_on_the_button() {
    let match_ = live_match();
    let events = std::slice::from_ref(&match_);
    let mut memory = Memory::default();
    let blank = need_scan(events, &memory, NOW, false, 0.0, &[]);
    memory.note_row(
        &scan_row(
            "p:11",
            "ok",
            Some(reading("Lens", "Lille", "direct", 0.9)),
            1,
        ),
        events,
        NOW,
    );

    let cached = need_scan(events, &memory, NOW + 500.0, false, 0.0, &[]);
    let forced = need_scan(events, &memory, NOW + 500.0, true, 0.0, &[]);
    let burst = need_scan(events, &memory, NOW + 500.0, true, NOW + 495.0, &[]);
    let other_league = need_scan(
        &[{
            let mut e = live_match();
            e.code = "eng.1".into();
            e
        }],
        &Memory::default(),
        NOW,
        false,
        0.0,
        &[],
    );
    let before_kickoff = need_scan(
        &[event("foot:1", "A", "B", State::Pre, NOW + 600.0)],
        &Memory::default(),
        NOW,
        false,
        0.0,
        &[],
    );
    let just_scanned = need_scan(events, &Memory::default(), NOW, false, NOW - 30.0, &[]);

    assert!(blank.multiplex && forced.multiplex);
    assert!(
        !cached.multiplex
            && !burst.multiplex
            && !other_league.multiplex
            && !before_kickoff.multiplex
            && !just_scanned.multiplex
    );
}

#[test]
fn the_cadence_follows_what_is_played_and_who_watches() {
    let live = live_match();
    let soon = event("foot:2", "A", "B", State::Pre, NOW + 600.0);

    assert_eq!(
        cadence_seconds(std::slice::from_ref(&live), true, NOW),
        45.0
    );
    assert_eq!(
        cadence_seconds(std::slice::from_ref(&live), false, NOW),
        900.0
    );
    assert_eq!(cadence_seconds(&[soon], true, NOW), 90.0);
    assert_eq!(cadence_seconds(&[], true, NOW), 300.0);
}
