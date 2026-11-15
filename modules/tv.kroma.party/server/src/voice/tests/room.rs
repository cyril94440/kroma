use std::collections::BTreeSet;

use super::super::error::VoiceError;
use super::super::limits::{SPEAKING_TTL_MS, TTL_MS};
use super::super::room::{JoinRequest, VoiceRoom};

const T0: i64 = 1_700_000_000_000;
const GRACE: i64 = 45_000;

fn room() -> VoiceRoom {
    VoiceRoom::new(T0)
}

fn request<'a>(user: &'a str, device: &'a str) -> JoinRequest<'a> {
    JoinRequest { user_id: user, name: user, device_id: device, muted: false, watching: false, max_peers: 8 }
}

fn join(room: &mut VoiceRoom, user: &str, device: &str) -> String {
    room.join(&request(user, device), T0).unwrap().peer_id
}

fn present(users: &[&str]) -> BTreeSet<String> {
    users.iter().map(|u| (*u).to_string()).collect()
}

#[test]
fn a_join_mints_a_distinct_peer_id_and_bumps_the_revision() {
    let mut room = room();
    let before = room.revision();

    let first = join(&mut room, "mamie", "device-aaaa");
    let second = join(&mut room, "papi", "device-bbbb");

    assert_ne!(first, second);
    assert_eq!(room.revision(), before + 2);
    assert_eq!(room.members().len(), 2);
}

#[test]
fn a_second_join_from_the_same_device_replaces_the_first_one() {
    let mut room = room();
    let first = join(&mut room, "mamie", "device-aaaa");

    let again = room.join(&request("mamie", "device-aaaa"), T0 + 1).unwrap();

    assert_eq!(again.replaced.as_deref(), Some(first.as_str()));
    assert_ne!(again.peer_id, first);
    assert_eq!(room.members().len(), 1);
    assert!(room.find(&first).is_none());
}

#[test]
fn two_devices_of_one_account_are_two_participants() {
    let mut room = room();

    let phone = join(&mut room, "mamie", "device-phone");
    let laptop = join(&mut room, "mamie", "device-laptop");

    assert_ne!(phone, laptop);
    assert_eq!(room.members().len(), 2);
    assert!(room.members().iter().all(|m| m.user_id == "mamie"));
}

#[test]
fn the_same_device_id_under_two_accounts_does_not_collide() {
    let mut room = room();

    let mamie = join(&mut room, "mamie", "device-aaaa");
    let papi = join(&mut room, "papi", "device-aaaa");

    assert_ne!(mamie, papi);
    assert_eq!(room.members().len(), 2);
}

#[test]
fn refuses_a_device_id_that_is_short_or_holds_odd_characters() {
    let mut room = room();

    for device in ["short", "has space in it", "slash/slash/slash", ""] {
        assert_eq!(room.join(&request("mamie", device), T0), Err(VoiceError::BadDevice), "{device}");
    }
    assert!(room.members().is_empty());
}

#[test]
fn refuses_a_new_device_when_the_room_is_full_but_lets_a_joined_one_rejoin() {
    let mut room = room();
    let small = |user, device| JoinRequest { max_peers: 2, ..request(user, device) };
    room.join(&small("mamie", "device-aaaa"), T0).unwrap();
    room.join(&small("papi", "device-bbbb"), T0).unwrap();

    let stranger = room.join(&small("cyril", "device-cccc"), T0);
    let rejoin = room.join(&small("papi", "device-bbbb"), T0);

    assert_eq!(stranger, Err(VoiceError::Full));
    assert!(rejoin.is_ok());
    assert_eq!(room.members().len(), 2);
}

#[test]
fn throttles_an_account_that_joins_in_a_loop() {
    let mut room = room();

    let outcomes: Vec<_> = (0..13).map(|_| room.join(&request("mamie", "device-aaaa"), T0).is_ok()).collect();

    assert_eq!(outcomes.iter().filter(|ok| **ok).count(), 12);
    assert_eq!(room.join(&request("mamie", "device-aaaa"), T0), Err(VoiceError::RateLimited));
    assert!(room.join(&request("mamie", "device-aaaa"), T0 + 61_000).is_ok());
}

#[test]
fn leaving_removes_the_participant_and_leaving_twice_is_harmless() {
    let mut room = room();
    let peer = join(&mut room, "mamie", "device-aaaa");

    let first = room.leave("mamie", &peer);
    let second = room.leave("mamie", &peer);

    assert_eq!((first, second), (Ok(true), Ok(false)));
    assert!(room.members().is_empty());
}

#[test]
fn nobody_can_make_another_account_leave() {
    let mut room = room();
    let peer = join(&mut room, "mamie", "device-aaaa");

    let refused = room.leave("papi", &peer);

    assert_eq!(refused, Err(VoiceError::Forbidden));
    assert_eq!(room.members().len(), 1);
}

#[test]
fn a_participant_without_a_beat_expires_after_the_ttl_and_a_beat_keeps_one_alive() {
    let mut room = room();
    let quiet = join(&mut room, "mamie", "device-aaaa");
    let alive = join(&mut room, "papi", "device-bbbb");
    room.beat("papi", &alive, T0 + TTL_MS - 1).unwrap();

    let removed = room.expire(T0 + TTL_MS, None, GRACE);

    assert_eq!(removed.iter().map(|p| p.peer_id.clone()).collect::<Vec<_>>(), vec![quiet]);
    assert_eq!(room.members().len(), 1);
}

#[test]
fn a_beat_for_an_unknown_peer_says_not_joined_so_the_client_rejoins() {
    let mut room = room();

    assert_eq!(room.beat("mamie", "gone-1", T0), Err(VoiceError::NotJoined));
}

#[test]
fn a_viewer_who_left_the_presence_list_is_dropped_after_the_grace() {
    let mut room = room();
    let request = JoinRequest { watching: true, ..request("mamie", "device-aaaa") };
    let peer = room.join(&request, T0).unwrap().peer_id;
    room.beat("mamie", &peer, T0 + 40_000).unwrap();
    let nobody = present(&[]);

    let early = room.expire(T0 + 10_000, Some(&nobody), GRACE);
    room.beat("mamie", &peer, T0 + 50_000).unwrap();
    let late = room.expire(T0 + 10_000 + GRACE, Some(&nobody), GRACE);

    assert!(early.is_empty());
    assert_eq!(late.len(), 1);
}

#[test]
fn coming_back_to_the_presence_list_cancels_the_pending_drop() {
    let mut room = room();
    let request = JoinRequest { watching: true, ..request("mamie", "device-aaaa") };
    let peer = room.join(&request, T0).unwrap().peer_id;
    room.expire(T0 + 1_000, Some(&present(&[])), GRACE);
    room.beat("mamie", &peer, T0 + 20_000).unwrap();
    room.expire(T0 + 20_000, Some(&present(&["mamie"])), GRACE);

    room.beat("mamie", &peer, T0 + 60_000).unwrap();
    let removed = room.expire(T0 + 60_000, Some(&present(&[])), GRACE);

    assert!(removed.is_empty());
}

#[test]
fn a_participant_who_is_not_watching_ignores_the_presence_list() {
    let mut room = room();
    let peer = join(&mut room, "mamie", "device-aaaa");
    room.beat("mamie", &peer, T0 + 200_000).unwrap();

    let removed = room.expire(T0 + 200_000, Some(&present(&[])), GRACE);

    assert!(removed.is_empty());
}

#[test]
fn an_unreadable_presence_list_drops_nobody() {
    let mut room = room();
    let request = JoinRequest { watching: true, ..request("mamie", "device-aaaa") };
    let peer = room.join(&request, T0).unwrap().peer_id;
    room.beat("mamie", &peer, T0 + 100_000).unwrap();

    let removed = room.expire(T0 + 100_000, None, GRACE);

    assert!(removed.is_empty());
}

#[test]
fn relays_number_each_ordered_pair_on_its_own() {
    let mut room = room();
    let mamie = join(&mut room, "mamie", "device-aaaa");
    let papi = join(&mut room, "papi", "device-bbbb");

    let first = room.relay("mamie", &mamie, &papi, T0).unwrap();
    let second = room.relay("mamie", &mamie, &papi, T0).unwrap();
    let reverse = room.relay("papi", &papi, &mamie, T0).unwrap();

    assert_eq!((first.n, second.n, reverse.n), (1, 2, 1));
    assert_eq!(first.to_user, "papi");
}

#[test]
fn the_pair_counter_restarts_when_the_other_side_rejoins() {
    let mut room = room();
    let mamie = join(&mut room, "mamie", "device-aaaa");
    let papi = join(&mut room, "papi", "device-bbbb");
    room.relay("mamie", &mamie, &papi, T0).unwrap();
    let papi_again = room.join(&request("papi", "device-bbbb"), T0 + 1).unwrap().peer_id;

    let relay = room.relay("mamie", &mamie, &papi_again, T0 + 2).unwrap();

    assert_eq!(relay.n, 1);
    assert_eq!(room.relay("mamie", &mamie, &papi, T0 + 2), Err(VoiceError::PeerGone));
}

#[test]
fn a_signal_from_someone_who_is_not_a_participant_is_refused() {
    let mut room = room();
    let papi = join(&mut room, "papi", "device-bbbb");

    let refused = room.relay("mamie", "ghost-1", &papi, T0);

    assert_eq!(refused, Err(VoiceError::NotJoined));
}

#[test]
fn a_participant_cannot_signal_as_someone_elses_peer() {
    let mut room = room();
    let mamie = join(&mut room, "mamie", "device-aaaa");
    let papi = join(&mut room, "papi", "device-bbbb");

    let refused = room.relay("papi", &mamie, &papi, T0);

    assert_eq!(refused, Err(VoiceError::Forbidden));
}

#[test]
fn a_signal_to_a_peer_that_left_says_peer_gone() {
    let mut room = room();
    let mamie = join(&mut room, "mamie", "device-aaaa");
    let papi = join(&mut room, "papi", "device-bbbb");
    room.leave("papi", &papi).unwrap();

    assert_eq!(room.relay("mamie", &mamie, &papi, T0), Err(VoiceError::PeerGone));
}

#[test]
fn a_signal_to_oneself_is_refused() {
    let mut room = room();
    let mamie = join(&mut room, "mamie", "device-aaaa");

    assert_eq!(room.relay("mamie", &mamie, &mamie, T0), Err(VoiceError::SelfSignal));
}

#[test]
fn a_flood_of_signals_is_throttled_per_sender_and_recovers() {
    let mut room = room();
    let mamie = join(&mut room, "mamie", "device-aaaa");
    let papi = join(&mut room, "papi", "device-bbbb");

    let sent = (0..250).filter(|_| room.relay("mamie", &mamie, &papi, T0).is_ok()).count();
    let reverse = room.relay("papi", &papi, &mamie, T0);
    let later = room.relay("mamie", &mamie, &papi, T0 + 10_000);

    assert_eq!(sent, 240);
    assert!(reverse.is_ok());
    assert!(later.is_ok());
}

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

    let accepted = (0..12).filter(|_| room.set_speaking("mamie", &mamie, true, T0).is_ok()).count();
    let silence = room.set_speaking("mamie", &mamie, false, T0);

    assert_eq!(accepted, 10);
    assert!(silence.is_ok());
    assert!(room.speaking(T0).is_empty());
}
