use std::sync::Arc;

use anyhow::Result;
use kroma_module_sdk::host::testing::StubHost;
use serde_json::{json, Value};

use super::support::{rig, rig_with_turn, Rig};
use crate::party::Actor;
use crate::voice::ice::IceServer;
use crate::voice::{Signal, TurnSource, VoiceError};

struct Relay;

impl TurnSource for Relay {
    fn mint(&self, _key_id: &str, _api_token: &str, _ttl_s: u32) -> Result<Vec<IceServer>> {
        Ok(vec![IceServer {
            urls: vec!["turn:relay.example:3478".into()],
            username: Some("minted-user".into()),
            credential: Some("minted-secret".into()),
        }])
    }
}

fn actor(name: &str) -> Actor {
    Actor {
        person: super::support::person(name),
        admin: false,
    }
}

fn join(rig: &Rig, name: &str, device: &str) -> String {
    let answer = rig
        .party
        .voice_join(&actor(name), device, false, false)
        .unwrap();
    answer["peerId"].as_str().unwrap().to_string()
}

fn description(from: &str, to: &str) -> Signal {
    serde_json::from_value(json!({
        "from": from, "to": to, "session": "s1", "kind": "description",
        "description": { "type": "offer", "sdp": "v=0" }
    }))
    .unwrap()
}

fn addressed(rig: &Rig) -> Vec<(String, Value)> {
    rig.all_events()
        .into_iter()
        .filter(|(to, topic, _)| to.is_some() && topic == "party.voice.signal")
        .map(|(to, _, payload)| (to.unwrap(), payload))
        .collect()
}

#[test]
fn a_join_announces_the_roster_without_a_sequence_and_says_so_once_in_the_chat() {
    let rig = rig();
    let seq = rig.party.lock().seq;

    let peer = join(&rig, "Mamie", "device-aaaa");

    let events = rig.events("party.voice.roster");
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0]["participants"],
        json!([{ "peerId": peer, "userId": "mamie", "name": "Mamie", "muted": false }])
    );
    assert!(events[0]["revision"].is_i64());
    assert!(events[0].get("seq").is_none());
    assert_eq!(rig.party.lock().seq, seq + 1);
    assert_eq!(rig.keys(), vec!["party.voice.joined"]);
}

#[test]
fn the_join_answer_carries_the_ice_servers_and_the_beat_schedule() {
    let rig = rig();

    let answer = rig
        .party
        .voice_join(&actor("Mamie"), "device-aaaa", false, false)
        .unwrap();

    assert_eq!(
        answer["iceServers"],
        json!([{ "urls": ["stun:stun.l.google.com:19302"] }])
    );
    assert_eq!(
        (
            answer["beatSeconds"].clone(),
            answer["expireSeconds"].clone()
        ),
        (json!(10), json!(30))
    );
    assert_eq!(answer["participants"].as_array().unwrap().len(), 1);
}

#[test]
fn minted_turn_credentials_reach_a_participant_and_never_an_event_or_the_state() {
    let host = StubHost::with_db("party")
        .with_config("voiceCloudflareKeyId", json!("key-1"))
        .with_config("voiceCloudflareApiToken", json!("token-1"));
    let rig = rig_with_turn(host, Arc::new(Relay));

    let answer = rig
        .party
        .voice_join(&actor("Mamie"), "device-aaaa", false, false)
        .unwrap();
    let state = rig.party.voice_state().to_string();
    let events = serde_json::to_string(&rig.all_events()).unwrap();

    assert_eq!(answer["iceServers"][0]["credential"], "minted-secret");
    for leaked in ["minted-secret", "minted-user", "token-1"] {
        assert!(
            !state.contains(leaked) && !events.contains(leaked),
            "{leaked}"
        );
    }
}

#[test]
fn only_a_participant_may_ask_for_ice_servers() {
    let rig = rig();
    join(&rig, "Mamie", "device-aaaa");

    let stranger = rig.party.voice_ice("papi");
    let member = rig.party.voice_ice("mamie");

    assert_eq!(stranger.unwrap_err(), VoiceError::NotJoined);
    assert!(member.unwrap()["iceServers"].is_array());
}

#[test]
fn a_signal_is_addressed_to_the_owner_of_the_target_peer_and_to_nobody_else() {
    let rig = rig();
    let mamie = join(&rig, "Mamie", "device-aaaa");
    let papi = join(&rig, "Papi", "device-bbbb");
    join(&rig, "Cyril", "device-cccc");

    let n = rig
        .party
        .voice_signal("mamie", &description(&mamie, &papi))
        .unwrap();

    let sent = addressed(&rig);
    assert_eq!(n, 1);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, "papi");
    assert_eq!(
        sent[0].1,
        json!({
            "to": papi, "from": mamie, "fromUserId": "mamie", "session": "s1", "n": 1,
            "kind": "description", "description": { "type": "offer", "sdp": "v=0" }
        })
    );
    assert!(rig.events("party.voice.signal").is_empty());
}

#[test]
fn a_refused_signal_is_relayed_to_nobody() {
    let rig = rig();
    let mamie = join(&rig, "Mamie", "device-aaaa");
    let papi = join(&rig, "Papi", "device-bbbb");

    let outsider = rig.party.voice_signal("cyril", &description(&mamie, &papi));
    let forged = rig.party.voice_signal("papi", &description(&mamie, &papi));
    let missing = rig
        .party
        .voice_signal("mamie", &description(&mamie, "ghost-1"));

    assert_eq!(outsider, Err(VoiceError::Forbidden));
    assert_eq!(forged, Err(VoiceError::Forbidden));
    assert_eq!(missing, Err(VoiceError::PeerGone));
    assert!(addressed(&rig).is_empty());
}

#[test]
fn a_user_who_never_joined_cannot_signal() {
    let rig = rig();
    let papi = join(&rig, "Papi", "device-bbbb");

    let refused = rig
        .party
        .voice_signal("mamie", &description("nobody-1", &papi));

    assert_eq!(refused, Err(VoiceError::NotJoined));
    assert!(addressed(&rig).is_empty());
}

#[test]
fn muting_and_leaving_each_announce_a_new_roster() {
    let rig = rig();
    let peer = join(&rig, "Mamie", "device-aaaa");

    rig.party.voice_mute("mamie", &peer, true).unwrap();
    rig.party.voice_mute("mamie", &peer, true).unwrap();
    rig.party.voice_leave("mamie", &peer).unwrap();

    let rosters = rig.events("party.voice.roster");
    assert_eq!(rosters.len(), 3);
    assert_eq!(rosters[1]["participants"][0]["muted"], true);
    assert_eq!(rosters[2]["participants"], json!([]));
}

#[test]
fn speaking_is_broadcast_as_the_full_set_and_clears_by_itself_after_the_ttl() {
    let rig = rig();
    let mamie = join(&rig, "Mamie", "device-aaaa");

    rig.party.voice_speaking("mamie", &mamie, true).unwrap();
    rig.party.voice_speaking("mamie", &mamie, true).unwrap();
    rig.advance(5_001);
    rig.party.voice_tick();

    let sets: Vec<Value> = rig
        .events("party.voice.speaking")
        .iter()
        .map(|e| e["speaking"].clone())
        .collect();
    assert_eq!(sets, vec![json!([mamie]), json!([])]);
}

#[test]
fn a_silent_participant_is_removed_by_the_tick_and_the_roster_says_so() {
    let rig = rig();
    let mamie = join(&rig, "Mamie", "device-aaaa");
    let papi = join(&rig, "Papi", "device-bbbb");
    rig.advance(20_000);
    rig.party.voice_beat("papi", &papi).unwrap();

    rig.advance(11_000);
    rig.party.voice_tick();

    let rosters = rig.events("party.voice.roster");
    let last = rosters.last().unwrap();
    assert_eq!(last["participants"].as_array().unwrap().len(), 1);
    assert_eq!(last["participants"][0]["userId"], "papi");
    assert_eq!(
        rig.party.voice_beat("mamie", &mamie),
        Err(VoiceError::NotJoined)
    );
}

#[test]
fn a_viewer_who_stops_watching_leaves_the_voice_room_after_the_grace() {
    let rig = rig();
    rig.present(&["Mamie"]);
    rig.party.refresh_presence();
    let peer = join_watching(&rig, "Mamie", "device-aaaa");
    rig.present(&[]);
    rig.party.refresh_presence();

    rig.advance(10_000);
    rig.party.voice_beat("mamie", &peer).unwrap();
    rig.party.voice_tick();
    let still_there = rig.party.voice_state()["participants"]
        .as_array()
        .unwrap()
        .len();
    rig.advance(46_000);
    rig.party.voice_beat("mamie", &peer).unwrap();
    rig.party.voice_tick();

    assert_eq!(still_there, 1);
    assert!(rig.party.voice_state()["participants"]
        .as_array()
        .unwrap()
        .is_empty());
}

fn join_watching(rig: &Rig, name: &str, device: &str) -> String {
    rig.party
        .voice_join(&actor(name), device, false, true)
        .unwrap()["peerId"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn the_state_lists_participants_and_who_is_speaking_for_a_late_joiner() {
    let rig = rig();
    let mamie = join(&rig, "Mamie", "device-aaaa");
    rig.party.voice_speaking("mamie", &mamie, true).unwrap();

    let state = rig.party.voice_state();

    assert_eq!(state["participants"][0]["peerId"], json!(mamie));
    assert_eq!(state["speaking"], json!([mamie]));
    assert_eq!(state["max"], 8);
    assert!(state["revision"].is_i64());
}

#[test]
fn the_configured_room_size_caps_the_mesh() {
    let rig =
        super::support::rig_with(StubHost::with_db("party").with_config("voiceMaxPeers", json!(2)));
    join(&rig, "Mamie", "device-aaaa");
    join(&rig, "Papi", "device-bbbb");

    let third = rig
        .party
        .voice_join(&actor("Cyril"), "device-cccc", false, false);

    assert_eq!(third.unwrap_err(), VoiceError::Full);
    assert_eq!(rig.party.voice_state()["max"], 2);
}

#[test]
fn leaving_says_so_in_the_chat_but_a_reload_replacing_the_entry_stays_silent() {
    let rig = rig();
    join(&rig, "Mamie", "device-aaaa");
    let again = join(&rig, "Mamie", "device-aaaa");

    rig.party.voice_leave("mamie", &again).unwrap();

    assert_eq!(rig.keys(), vec!["party.voice.joined", "party.voice.left"]);
}
