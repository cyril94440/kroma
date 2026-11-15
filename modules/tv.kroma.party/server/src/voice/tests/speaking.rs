use super::super::limits::SPEAKING_TTL_MS;
use super::support::*;

#[test]
fn speaking_lasts_the_ttl_unless_refreshed_and_is_announced_once_per_change() {
    let mut room = room();
    let mamie = join(&mut room, "mamie", "device-aaaa");

    room.set_speaking("mamie", &mamie, true, T0).unwrap();
    let started = room.speaking_change(T0);
    let unchanged = room.speaking_change(T0 + 1_000);
    let stopped = room.speaking_change(T0 + SPEAKING_TTL_MS);

    assert_eq!(started, Some(vec![mamie]));
    assert_eq!(unchanged, None);
    assert_eq!(stopped, Some(vec![]));
}

#[test]
fn a_muted_participant_is_never_marked_speaking_and_muting_clears_it() {
    let mut room = room();
    let mamie = join(&mut room, "mamie", "device-aaaa");
    room.set_speaking("mamie", &mamie, true, T0).unwrap();

    let changed = room.set_muted("mamie", &mamie, true, T0 + 1).unwrap();
    room.set_speaking("mamie", &mamie, true, T0 + 2).unwrap();

    assert!(changed);
    assert!(room.speaking(T0 + 3).is_empty());
}

#[test]
fn muting_twice_changes_nothing_and_leaves_the_revision_alone() {
    let mut room = room();
    let mamie = join(&mut room, "mamie", "device-aaaa");
    room.set_muted("mamie", &mamie, true, T0).unwrap();
    let revision = room.revision();

    let changed = room.set_muted("mamie", &mamie, true, T0).unwrap();

    assert!(!changed);
    assert_eq!(room.revision(), revision);
}

#[test]
fn speaking_is_rate_limited_but_silence_always_goes_through() {
    let mut room = room();
    let mamie = join(&mut room, "mamie", "device-aaaa");

    let accepted = (0..12)
        .filter(|_| room.set_speaking("mamie", &mamie, true, T0).is_ok())
        .count();
    let silence = room.set_speaking("mamie", &mamie, false, T0);

    assert_eq!(accepted, 10);
    assert!(silence.is_ok());
    assert!(room.speaking(T0).is_empty());
}
