use serde_json::{json, Value};

use super::support::{rig, Rig};
use super::voice::join;
use crate::voice::{Signal, VoiceError};

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
