use super::super::error::VoiceError;
use super::support::*;

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
    let papi_again = room
        .join(&request("papi", "device-bbbb"), T0 + 1)
        .unwrap()
        .peer_id;

    let relay = room.relay("mamie", &mamie, &papi_again, T0 + 2).unwrap();

    assert_eq!(relay.n, 1);
    assert_eq!(
        room.relay("mamie", &mamie, &papi, T0 + 2),
        Err(VoiceError::PeerGone)
    );
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
    room.leave("papi", &papi, false, T0).unwrap();

    assert_eq!(
        room.relay("mamie", &mamie, &papi, T0),
        Err(VoiceError::PeerGone)
    );
}

#[test]
fn a_signal_to_oneself_is_refused() {
    let mut room = room();
    let mamie = join(&mut room, "mamie", "device-aaaa");

    assert_eq!(
        room.relay("mamie", &mamie, &mamie, T0),
        Err(VoiceError::SelfSignal)
    );
}

#[test]
fn a_flood_of_signals_is_throttled_per_sender_and_recovers() {
    let mut room = room();
    let mamie = join(&mut room, "mamie", "device-aaaa");
    let papi = join(&mut room, "papi", "device-bbbb");

    let sent = (0..250)
        .filter(|_| room.relay("mamie", &mamie, &papi, T0).is_ok())
        .count();
    let reverse = room.relay("papi", &papi, &mamie, T0);
    let later = room.relay("mamie", &mamie, &papi, T0 + 10_000);

    assert_eq!(sent, 240);
    assert!(reverse.is_ok());
    assert!(later.is_ok());
}
