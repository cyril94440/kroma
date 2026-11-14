use serde_json::json;

use super::support::*;
use crate::party::Request;

fn tf1(rig: &Rig) {
    rig.live.names.lock().unwrap().insert("tf1".into(), "TF1".into());
}

#[test]
fn a_viewer_alone_switches_at_once_without_a_vote() {
    let rig = rig();
    tf1(&rig);
    rig.present(&["Cyril"]);
    let result = rig.ask("Cyril", "tf1");
    assert_eq!(result.outcome, "switchedDirectly");
    assert_eq!(rig.switched_to(), vec!["tf1"]);
    assert!(!rig.vote_open());
    assert_eq!(rig.last_system().key.as_deref(), Some("party.vote.solo"));
    assert_eq!(rig.last_system().params["channel"], "TF1");
    assert_eq!(rig.live.switches.lock().unwrap()[0].1, "cyril");
}

#[test]
fn others_watching_open_a_vote_that_the_state_and_the_events_show() {
    let rig = rig();
    tf1(&rig);
    rig.present(&["Cyril", "Mamie"]);
    let result = rig.ask("Cyril", "tf1");
    assert_eq!(result.outcome, "voteStarted");
    assert!(result.vote_id.is_some());
    assert!(rig.switched_to().is_empty());
    assert_eq!(rig.vote_required(), vec!["mamie"]);
    assert_eq!(rig.last_system().key.as_deref(), Some("party.vote.opened"));
    let events = rig.events("party.vote");
    assert_eq!(events.last().unwrap()["vote"]["label"], "TF1");
    assert_eq!(events.last().unwrap()["vote"]["tally"], json!({ "yes": 1, "no": 0, "blank": 1 }));
}

#[test]
fn the_vote_closes_as_soon_as_the_majority_is_acquired() {
    let rig = rig();
    tf1(&rig);
    rig.present(&["Cyril", "Mamie", "Papi", "Ced", "Lea"]);
    rig.ask("Cyril", "tf1");
    rig.party.cast_vote(&person("Mamie"), true).unwrap();
    assert!(rig.vote_open());
    rig.party.cast_vote(&person("Papi"), true).unwrap();
    assert!(!rig.vote_open());
    assert_eq!(rig.switched_to(), vec!["tf1"]);
    assert_eq!(rig.last_system().key.as_deref(), Some("party.vote.passed"));

    let rig = self::rig();
    tf1(&rig);
    rig.present(&["Cyril", "Mamie", "Papi", "Ced"]);
    rig.ask("Cyril", "tf1");
    rig.party.cast_vote(&person("Mamie"), false).unwrap();
    assert!(rig.vote_open());
    rig.party.cast_vote(&person("Papi"), false).unwrap();
    assert!(!rig.vote_open());
    assert!(rig.switched_to().is_empty());
    assert_eq!(rig.last_system().key.as_deref(), Some("party.vote.refused"));
}

#[test]
fn one_person_is_one_voice_and_a_stranger_cannot_join_the_vote() {
    let rig = rig();
    tf1(&rig);
    rig.present(&["Ced", "Cyril", "Mamie"]);
    rig.ask("Cyril", "tf1");
    assert_eq!(rig.party.cast_vote(&person("Cyril"), true), Err("requester"));
    rig.present(&["Ced", "Cyril", "Mamie", "Sebi"]);
    rig.party.cast_vote(&person("Sebi"), true).unwrap();
    assert_eq!(rig.vote_required(), vec!["ced", "mamie", "sebi"]);
    rig.party.cast_vote(&person("Sebi"), false).unwrap();
    assert_eq!(rig.party.lock().vote.as_ref().unwrap().votes.len(), 1);
    assert_eq!(rig.party.cast_vote(&person("Intrus"), true), Err("notPresent"));
    assert!(!rig.vote_required().contains(&"intrus".to_string()));
    rig.party.cast_vote(&person("Ced"), true).unwrap();
    rig.party.cast_vote(&person("Mamie"), true).unwrap();
    assert!(!rig.vote_open());
    assert_eq!(rig.switched_to(), vec!["tf1"]);
    let message = rig.last_system();
    assert_eq!((message.params["yes"].as_str(), message.params["no"].as_str()), (Some("3"), Some("1")));
}

#[test]
fn thirty_seconds_for_against_the_request_counts_for_and_a_tie_stays() {
    let rig = rig();
    tf1(&rig);
    rig.present(&["Benji", "Ced"]);
    rig.ask("Benji", "tf1");
    assert_eq!(rig.party.cast_vote(&person("Benji"), true), Err("requester"));
    rig.advance(29_000);
    rig.party.tick();
    assert!(rig.vote_open());
    rig.advance(1_001);
    rig.party.tick();
    assert!(!rig.vote_open());
    assert_eq!(rig.switched_to(), vec!["tf1"]);
    let message = rig.last_system();
    assert_eq!(message.key.as_deref(), Some("party.vote.passed"));
    assert_eq!((message.params["yes"].as_str(), message.params["no"].as_str()), (Some("1"), Some("0")));

    let rig = self::rig();
    tf1(&rig);
    rig.present(&["Benji", "Ced"]);
    rig.ask("Benji", "tf1");
    rig.party.cast_vote(&person("Ced"), false).unwrap();
    assert!(!rig.vote_open());
    assert!(rig.switched_to().is_empty());
    assert_eq!(rig.last_system().key.as_deref(), Some("party.vote.refused"));
}

#[test]
fn a_proposal_of_the_assistant_has_no_voice_of_its_own() {
    let rig = rig();
    tf1(&rig);
    rig.present(&["Benji", "Ced"]);
    let result = rig.party.request_switch(Request {
        channel_id: "tf1",
        channel_name: None,
        requester: None,
        note: "football",
        force: false,
    });
    assert_eq!(result.outcome, "voteStarted");
    assert_eq!(rig.last_system().key.as_deref(), Some("party.vote.openedByAgent"));
    rig.advance(31_000);
    rig.party.tick();
    assert!(rig.switched_to().is_empty());
}

#[test]
fn an_unknown_presence_switches_nobody_and_keeps_an_open_vote() {
    let rig = rig();
    tf1(&rig);
    rig.present(&["Cyril", "Mamie", "Papi"]);
    assert_eq!(rig.ask("Cyril", "tf1").outcome, "voteStarted");
    rig.unreachable();
    rig.party.reevaluate_vote();
    assert!(rig.vote_open());
    assert_eq!(rig.vote_required(), vec!["mamie", "papi"]);
    assert!(rig.switched_to().is_empty());
    rig.present(&["Cyril", "Mamie", "Papi"]);
    rig.party.cast_vote(&person("Mamie"), false).unwrap();
    rig.party.cast_vote(&person("Papi"), false).unwrap();

    let rig = self::rig();
    tf1(&rig);
    rig.unreachable();
    let result = rig.ask("Cyril", "tf1");
    assert_eq!((result.outcome, result.reason.as_deref()), ("refused", Some("presenceUnavailable")));
    assert!(rig.switched_to().is_empty());
}

#[test]
fn an_empty_presence_reading_once_keeps_the_quorum_and_twice_cancels_when_the_requester_left() {
    let rig = rig_no_grace();
    tf1(&rig);
    rig.present(&["Cyril", "Mamie"]);
    rig.ask("Cyril", "tf1");
    rig.present(&[]);
    rig.party.reevaluate_vote();
    assert!(rig.vote_open());
    rig.party.reevaluate_vote();
    assert!(!rig.vote_open());
    assert!(rig.switched_to().is_empty());
    assert!(!rig.keys().contains(&"party.vote.alone".to_string()));
    assert_eq!(rig.events("party.vote").last().unwrap()["closed"]["verdict"], "cancelled");
}

#[test]
fn the_requester_staying_after_the_voters_left_switches_as_alone() {
    let rig = rig_no_grace();
    tf1(&rig);
    rig.present(&["Cyril", "Mamie"]);
    rig.ask("Cyril", "tf1");
    rig.present(&["Cyril"]);
    rig.party.reevaluate_vote();
    assert!(rig.vote_open());
    rig.party.reevaluate_vote();
    assert!(!rig.vote_open());
    assert_eq!(rig.switched_to(), vec!["tf1"]);
    assert_eq!(rig.last_system().key.as_deref(), Some("party.vote.alone"));
}

#[test]
fn a_second_request_while_a_vote_is_open_is_refused_as_busy() {
    let rig = rig();
    tf1(&rig);
    rig.present(&["Cyril", "Mamie"]);
    let first = rig.ask("Cyril", "tf1");
    let second = rig.ask("Mamie", "m6");
    assert_eq!((second.outcome, second.reason.as_deref()), ("refused", Some("busy")));
    assert_eq!(second.vote_id, first.vote_id);
}

#[test]
fn the_channel_already_on_air_is_never_voted_on() {
    let rig = rig();
    tf1(&rig);
    rig.present(&["Cyril", "Mamie"]);
    rig.live.status.lock().unwrap().channel = Some(crate::live::ChannelRef { id: "tf1".into(), name: "TF1".into() });
    let result = rig.ask("Cyril", "tf1");
    assert_eq!((result.outcome, result.reason.as_deref()), ("refused", Some("alreadyOn")));
    assert!(!rig.vote_open());
}

#[test]
fn a_failed_switch_after_a_won_vote_is_said_and_recorded_as_impossible() {
    let rig = rig();
    tf1(&rig);
    rig.present(&["Cyril", "Mamie"]);
    *rig.live.switch_error.lock().unwrap() = Some("offAir".into());
    rig.ask("Cyril", "tf1");
    rig.advance(31_000);
    rig.party.tick();
    assert_eq!(rig.last_system().key.as_deref(), Some("party.vote.switchFailed"));
    assert_eq!(rig.events("party.vote").last().unwrap()["closed"]["verdict"], "impossible");
    assert_eq!(rig.party.recent_votes(5)[0]["verdict"], "impossible");
}

#[test]
fn forcing_goes_through_without_a_vote_and_the_bypass_list_is_configurable() {
    let rig = rig_with(kroma_module_sdk::host::testing::StubHost::with_db("party-bypass").with_config("bypassUserIds", json!("papi, lea")));
    tf1(&rig);
    rig.present(&["Cyril", "Mamie"]);
    let admin = crate::party::Actor { person: person("Cyril"), admin: true };
    let plain = crate::party::Actor { person: person("Mamie"), admin: false };
    let listed = crate::party::Actor { person: person("Papi"), admin: false };
    assert!(rig.party.may_bypass(&admin));
    assert!(!rig.party.may_bypass(&plain));
    assert!(rig.party.may_bypass(&listed));
    let result = rig.party.request_switch(Request {
        channel_id: "tf1",
        channel_name: None,
        requester: Some(person("Cyril")),
        note: "",
        force: true,
    });
    assert_eq!(result.outcome, "switchedDirectly");
    assert_eq!(rig.last_system().key.as_deref(), Some("party.vote.forced"));
    assert!(!rig.vote_open());
}

#[test]
fn a_typed_yes_or_no_in_the_chat_is_counted_as_a_vote() {
    let rig = rig();
    tf1(&rig);
    rig.present(&["Cyril", "Mamie", "Papi", "Ced"]);
    let admin = |n: &str| crate::party::Actor { person: person(n), admin: false };
    rig.ask("Cyril", "tf1");
    rig.party.post_message(&admin("Mamie"), "normal, on reste").unwrap();
    assert!(rig.party.lock().vote.as_ref().unwrap().votes.is_empty());
    rig.party.post_message(&admin("Mamie"), "non merci").unwrap();
    assert_eq!(rig.party.lock().vote.as_ref().unwrap().votes.get("mamie"), Some(&false));
    rig.party.post_message(&admin("Cyril"), "oui oui oui").unwrap();
    assert_eq!(rig.party.lock().vote.as_ref().unwrap().votes.len(), 1);
    rig.party.post_message(&admin("Papi"), "oui").unwrap();
    rig.party.post_message(&admin("Ced"), "ok go").unwrap();
    assert_eq!(rig.switched_to(), vec!["tf1"]);
}

#[test]
fn the_vote_history_keeps_every_closed_vote() {
    let rig = rig();
    tf1(&rig);
    rig.present(&["Cyril", "Mamie"]);
    rig.ask("Cyril", "tf1");
    rig.party.cast_vote(&person("Mamie"), false).unwrap();
    let rows = rig.party.recent_votes(10);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["verdict"], "refused");
    assert_eq!(rows[0]["by"], "Cyril");
}
