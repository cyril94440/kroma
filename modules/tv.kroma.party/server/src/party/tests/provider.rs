use serde_json::json;

use super::support::*;

#[test]
fn a_provider_proposal_of_the_live_module_is_voted_and_answered() {
    let rig = rig();
    rig.present(&["Cyril", "Mamie"]);
    rig.party.on_proposal(&json!({ "id": "p1", "kind": "switchToBackup" }));
    assert!(rig.vote_open());
    assert_eq!(rig.last_system().key.as_deref(), Some("party.vote.providerOpened.switchToBackup"));
    rig.party.on_proposal(&json!({ "id": "p1", "kind": "switchToBackup" }));
    assert!(rig.live.proposals.lock().unwrap().is_empty());
    rig.party.on_proposal(&json!({ "id": "p2", "kind": "returnToPrimary" }));
    assert_eq!(rig.live.proposals.lock().unwrap().as_slice(), &[("p2".to_string(), false)]);
    rig.party.cast_vote(&person("Cyril"), true).unwrap();
    rig.party.cast_vote(&person("Mamie"), true).unwrap();
    assert_eq!(rig.live.proposals.lock().unwrap().last(), Some(&("p1".to_string(), true)));
    assert_eq!(rig.last_system().key.as_deref(), Some("party.vote.providerAccepted"));
}

#[test]
fn a_provider_proposal_refused_by_the_family_is_declined_and_an_empty_room_accepts() {
    let rig = rig();
    rig.present(&["Cyril", "Mamie"]);
    rig.party.on_proposal(&json!({ "id": "p1", "kind": "switchToBackup" }));
    rig.party.cast_vote(&person("Cyril"), false).unwrap();
    assert_eq!(rig.live.proposals.lock().unwrap().last(), Some(&("p1".to_string(), false)));
    assert_eq!(rig.last_system().key.as_deref(), Some("party.vote.providerDeclined"));

    let rig = self::rig();
    rig.present(&[]);
    rig.party.on_proposal(&json!({ "id": "p3", "kind": "returnToPrimary" }));
    assert_eq!(rig.live.proposals.lock().unwrap().last(), Some(&("p3".to_string(), true)));
}

#[test]
fn a_malformed_proposal_is_ignored() {
    let rig = rig();
    rig.present(&["Cyril", "Mamie"]);
    rig.party.on_proposal(&json!({ "id": "", "kind": "switchToBackup" }));
    rig.party.on_proposal(&json!({ "id": "p", "kind": "unknown" }));
    assert!(!rig.vote_open());
}

#[test]
fn a_switch_provider_proposal_is_voted_like_the_other_two_kinds() {
    let rig = rig();
    rig.present(&["Cyril", "Mamie"]);
    rig.party.on_proposal(&json!({ "id": "p9", "kind": "switchProvider" }));
    assert!(rig.vote_open());
    assert_eq!(rig.last_system().key.as_deref(), Some("party.vote.providerOpened.switchProvider"));
    rig.party.cast_vote(&person("Cyril"), true).unwrap();
    rig.party.cast_vote(&person("Mamie"), true).unwrap();
    assert_eq!(rig.live.proposals.lock().unwrap().last(), Some(&("p9".to_string(), true)));
    assert_eq!(rig.last_system().key.as_deref(), Some("party.vote.providerAccepted"));
}

#[test]
fn a_switch_provider_proposal_the_family_refuses_is_declined() {
    let rig = rig();
    rig.present(&["Cyril", "Mamie"]);
    rig.party.on_proposal(&json!({ "id": "p9", "kind": "switchProvider" }));
    rig.party.cast_vote(&person("Cyril"), false).unwrap();
    assert_eq!(rig.live.proposals.lock().unwrap().last(), Some(&("p9".to_string(), false)));
}
