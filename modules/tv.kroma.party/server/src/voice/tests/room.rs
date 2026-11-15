use super::super::error::VoiceError;
use super::super::limits::TTL_MS;
use super::super::room::JoinRequest;
use super::support::*;

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
        assert_eq!(
            room.join(&request("mamie", device), T0),
            Err(VoiceError::BadDevice),
            "{device}"
        );
    }
    assert!(room.members().is_empty());
}

#[test]
fn refuses_a_new_device_when_the_room_is_full_but_lets_a_joined_one_rejoin() {
    let mut room = room();
    let small = |user, device| JoinRequest {
        max_peers: 2,
        ..request(user, device)
    };
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

    let outcomes: Vec<_> = (0..13)
        .map(|_| room.join(&request("mamie", "device-aaaa"), T0).is_ok())
        .collect();

    assert_eq!(outcomes.iter().filter(|ok| **ok).count(), 12);
    assert_eq!(
        room.join(&request("mamie", "device-aaaa"), T0),
        Err(VoiceError::RateLimited)
    );
    assert!(room
        .join(&request("mamie", "device-aaaa"), T0 + 61_000)
        .is_ok());
}

#[test]
fn leaving_removes_the_participant_and_leaving_twice_is_harmless() {
    let mut room = room();
    let peer = join(&mut room, "mamie", "device-aaaa");

    let first = room.leave("mamie", &peer, false, T0);
    let second = room.leave("mamie", &peer, false, T0);

    assert_eq!((first, second), (Ok(true), Ok(false)));
    assert!(room.members().is_empty());
}

#[test]
fn nobody_can_make_another_account_leave() {
    let mut room = room();
    let peer = join(&mut room, "mamie", "device-aaaa");

    let refused = room.leave("papi", &peer, false, T0);

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

    assert_eq!(
        removed
            .iter()
            .map(|p| p.peer_id.clone())
            .collect::<Vec<_>>(),
        vec![quiet]
    );
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
    let request = JoinRequest {
        watching: true,
        ..request("mamie", "device-aaaa")
    };
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
    let request = JoinRequest {
        watching: true,
        ..request("mamie", "device-aaaa")
    };
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
    let request = JoinRequest {
        watching: true,
        ..request("mamie", "device-aaaa")
    };
    let peer = room.join(&request, T0).unwrap().peer_id;
    room.beat("mamie", &peer, T0 + 100_000).unwrap();

    let removed = room.expire(T0 + 100_000, None, GRACE);

    assert!(removed.is_empty());
}

#[test]
fn a_quiet_leave_followed_by_a_return_within_the_grace_is_not_announced() {
    let mut room = room();
    let peer = join(&mut room, "mamie", "device-aaaa");
    room.leave("mamie", &peer, true, T0).unwrap();

    let soon = room
        .join(&request("mamie", "device-aaaa"), T0 + 60_000)
        .unwrap();
    room.leave("mamie", &soon.peer_id, true, T0 + 61_000)
        .unwrap();
    let late = room
        .join(&request("mamie", "device-aaaa"), T0 + 300_000)
        .unwrap();

    assert!(!soon.announce);
    assert!(late.announce);
}

#[test]
fn one_account_holds_at_most_two_slots_whatever_the_device_ids() {
    let mut room = room();
    join(&mut room, "mamie", "device-aaaa");
    join(&mut room, "mamie", "device-bbbb");

    let third = room.join(&request("mamie", "device-cccc"), T0);
    let rejoin = room.join(&request("mamie", "device-aaaa"), T0);
    let other = room.join(&request("papi", "device-cccc"), T0);

    assert_eq!(third, Err(VoiceError::AccountFull));
    assert!(rejoin.is_ok());
    assert!(other.is_ok());
    assert_eq!(room.members().iter().filter(|m| m.user_id == "mamie").count(), 2);
}

#[test]
fn a_slot_freed_by_leaving_lets_the_account_take_another_device() {
    let mut room = room();
    let first = join(&mut room, "mamie", "device-aaaa");
    join(&mut room, "mamie", "device-bbbb");
    room.leave("mamie", &first, false, T0).unwrap();

    assert!(room.join(&request("mamie", "device-cccc"), T0).is_ok());
}
