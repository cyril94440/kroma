use serde_json::json;

use super::support::*;

fn opened(id: &str, kind: &str) -> serde_json::Value {
    json!({
        "proposal": {
            "id": id,
            "kind": kind,
            "text": "unstable",
            "channel": "s:1",
            "createdAt": 1000,
            "expiresAt": 31000
        },
        "resolved": false,
        "viewers": 2
    })
}

fn resolved(id: &str, kind: &str, accepted: bool, default: &str) -> serde_json::Value {
    json!({
        "proposal": {
            "id": id,
            "kind": kind,
            "text": "unstable",
            "channel": "s:1",
            "createdAt": 1000,
            "expiresAt": 31000
        },
        "resolved": true,
        "accepted": accepted,
        "default": default
    })
}

#[test]
fn a_provider_proposal_of_the_live_module_is_voted_and_answered() {
    let rig = rig();
    rig.present(&["Cyril", "Mamie"]);
    rig.party.on_proposal(&opened("p1", "switchToBackup"));
    assert!(rig.vote_open());
    assert_eq!(rig.last_system().key.as_deref(), Some("party.vote.providerOpened.switchToBackup"));
    rig.party.on_proposal(&opened("p1", "switchToBackup"));
    assert!(rig.live.proposals.lock().unwrap().is_empty());
    rig.party.on_proposal(&opened("p2", "returnToPrimary"));
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
    rig.party.on_proposal(&opened("p1", "switchToBackup"));
    rig.party.cast_vote(&person("Cyril"), false).unwrap();
    assert_eq!(rig.live.proposals.lock().unwrap().last(), Some(&("p1".to_string(), false)));
    assert_eq!(rig.last_system().key.as_deref(), Some("party.vote.providerDeclined"));

    let rig = self::rig();
    rig.present(&[]);
    rig.party.on_proposal(&opened("p3", "returnToPrimary"));
    assert_eq!(rig.live.proposals.lock().unwrap().last(), Some(&("p3".to_string(), true)));
}

#[test]
fn a_malformed_proposal_is_ignored() {
    let rig = rig();
    rig.present(&["Cyril", "Mamie"]);
    rig.party.on_proposal(&opened("", "switchToBackup"));
    rig.party.on_proposal(&opened("p", "unknown"));
    assert!(!rig.vote_open());
}

#[test]
fn a_switch_provider_proposal_is_voted_like_the_other_two_kinds() {
    let rig = rig();
    rig.present(&["Cyril", "Mamie"]);
    rig.party.on_proposal(&opened("p9", "switchProvider"));
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
    rig.party.on_proposal(&opened("p9", "switchProvider"));
    rig.party.cast_vote(&person("Cyril"), false).unwrap();
    assert_eq!(rig.live.proposals.lock().unwrap().last(), Some(&("p9".to_string(), false)));
}

#[test]
fn the_exact_payload_live_publishes_for_an_opened_proposal_opens_a_vote() {
    let rig = rig();
    rig.present(&["Cyril", "Mamie"]);

    rig.party.on_proposal(&json!({
        "proposal": {
            "id": "prop-1",
            "kind": "switchToBackup",
            "text": "unstable",
            "channel": "s:1",
            "createdAt": 1000,
            "expiresAt": 31000
        },
        "resolved": false,
        "viewers": 0
    }));

    assert!(rig.vote_open());
    assert_eq!(rig.last_system().key.as_deref(), Some("party.vote.providerOpened.switchToBackup"));
}

#[test]
fn a_flat_payload_without_the_proposal_envelope_opens_nothing() {
    let rig = rig();
    rig.present(&["Cyril", "Mamie"]);

    rig.party.on_proposal(&json!({ "id": "p1", "kind": "switchToBackup" }));

    assert!(!rig.vote_open());
}

#[test]
fn a_proposal_resolved_by_live_before_the_vote_ends_closes_the_vote_without_answering_it() {
    let rig = rig();
    rig.present(&["Cyril", "Mamie"]);
    rig.party.on_proposal(&opened("p1", "switchProvider"));

    rig.party.on_proposal(&resolved("p1", "switchProvider", false, "timeout"));

    assert!(!rig.vote_open());
    assert!(rig.live.proposals.lock().unwrap().is_empty());
    assert_eq!(rig.last_system().key.as_deref(), Some("party.vote.providerExpired"));
}

#[test]
fn a_resolution_of_another_proposal_leaves_the_open_vote_alone() {
    let rig = rig();
    rig.present(&["Cyril", "Mamie"]);
    rig.party.on_proposal(&opened("p1", "switchProvider"));

    rig.party.on_proposal(&resolved("other", "switchToBackup", false, "timeout"));

    assert!(rig.vote_open());
}

#[test]
fn the_resolution_live_publishes_after_the_vote_answer_is_harmless() {
    let rig = rig();
    rig.present(&["Cyril", "Mamie"]);
    rig.party.on_proposal(&opened("p1", "switchProvider"));
    rig.party.cast_vote(&person("Cyril"), true).unwrap();
    rig.party.cast_vote(&person("Mamie"), true).unwrap();

    rig.party.on_proposal(&resolved("p1", "switchProvider", true, "vote"));

    assert_eq!(rig.last_system().key.as_deref(), Some("party.vote.providerAccepted"));
    assert!(!rig.vote_open());
}

fn requested(id: &str, by: &str) -> serde_json::Value {
    let mut event = opened(id, "switchProvider");
    event["proposal"]["requestedBy"] = json!(by);
    event
}

#[test]
fn the_exact_payload_live_publishes_for_a_requested_proposal_counts_the_asker_as_a_for() {
    let rig = rig();
    rig.present(&["Cyril", "Mamie"]);

    rig.party.on_proposal(&json!({
        "proposal": {
            "id": "prop-1",
            "kind": "switchProvider",
            "text": "requested",
            "channel": "s:1",
            "requestedBy": "cyril",
            "createdAt": 1000,
            "expiresAt": 31000
        },
        "resolved": false,
        "viewers": 0
    }));

    assert!(rig.vote_open());
}

#[test]
fn a_requested_proposal_left_to_its_deadline_with_blanks_passes_one_to_nothing() {
    let rig = rig();
    rig.present(&["Cyril", "Mamie"]);
    rig.party.on_proposal(&requested("p1", "cyril"));
    assert_eq!(rig.vote_required(), vec!["mamie".to_string()]);

    rig.advance(31_000);
    rig.party.tick();

    assert!(!rig.vote_open());
    assert_eq!(rig.live.proposals.lock().unwrap().last(), Some(&("p1".to_string(), true)));
}

#[test]
fn a_requester_who_is_alone_is_applied_at_once() {
    let rig = rig();
    rig.present(&["Cyril"]);
    rig.party.on_proposal(&requested("p1", "cyril"));
    assert!(!rig.vote_open());
    assert_eq!(rig.live.proposals.lock().unwrap().last(), Some(&("p1".to_string(), true)));
}

#[test]
fn an_asker_who_is_not_present_is_not_counted() {
    let rig = rig();
    rig.present(&["Cyril", "Mamie"]);
    rig.party.on_proposal(&requested("p1", "ghost"));
    assert!(rig.vote_open());
}
