use serde_json::json;

use super::support::*;
use crate::party::{IdeaError, IdeaInput};

fn input(text: &str) -> IdeaInput {
    IdeaInput { genre: "bug".into(), text: text.into(), ..IdeaInput::default() }
}

#[test]
fn an_idea_is_stored_trimmed_public_to_everyone_and_keeps_its_context_server_side() {
    let rig = rig();
    let id = rig
        .party
        .submit_idea(
            &person("Cyril"),
            IdeaInput { channel: Some("CANAL+ FOOT".into()), device: Some("Apple TV".into()), ..input("  wrong score  ") },
            false,
        )
        .unwrap();
    let all = rig.party.ideas_view();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0]["text"], "wrong score");
    assert_eq!(all[0]["status"], "received");
    assert_eq!(all[0]["author"], json!({ "userId": "cyril", "name": "Cyril" }));
    assert!(all[0].get("context").is_none());
    let triage = rig.party.ideas_for_triage(None);
    assert_eq!(triage[0]["context"]["channel"], "CANAL+ FOOT");
    assert_eq!(triage[0]["id"], json!(id));
    assert_eq!(rig.events("party.ideas").last().unwrap()["ideas"][0]["id"], json!(id));
}

#[test]
fn an_empty_idea_an_unknown_genre_and_a_burst_are_handled_like_the_source() {
    let rig = rig();
    assert_eq!(rig.party.submit_idea(&person("A"), input("   "), false), Err(IdeaError::Empty));
    rig.party.submit_idea(&person("A"), IdeaInput { genre: "whatever".into(), text: "x".into(), ..IdeaInput::default() }, false).unwrap();
    assert_eq!(rig.party.ideas_view()[0]["genre"], "other");
    assert_eq!(rig.party.submit_idea(&person("A"), input("y"), false), Err(IdeaError::TooFast));
    rig.advance(10_001);
    rig.party.submit_idea(&person("A"), input(&"y".repeat(5000)), false).unwrap();
    assert_eq!(rig.party.ideas_view()[0]["text"].as_str().unwrap().chars().count(), 1000);
}

#[test]
fn thirty_ideas_a_day_per_person_at_most() {
    let rig = rig();
    for _ in 0..30 {
        rig.party.submit_idea(&person("A"), input("idea"), false).unwrap();
        rig.advance(10_001);
    }
    assert_eq!(rig.party.submit_idea(&person("A"), input("one more"), false), Err(IdeaError::TooMany));
    assert!(rig.party.submit_idea(&person("B"), input("fine"), false).is_ok());
}

#[test]
fn the_assistant_may_file_several_ideas_in_a_row_with_a_source() {
    let rig = rig();
    let a = IdeaInput { source: Some("chat".into()), ..input("one") };
    rig.party.submit_idea(&person("Hermès"), a.clone(), true).unwrap();
    rig.party.submit_idea(&person("Hermès"), IdeaInput { text: "two".into(), ..a }, true).unwrap();
    let triage = rig.party.ideas_for_triage(None);
    assert_eq!(triage.len(), 2);
    assert_eq!(triage[1]["source"], "chat");
    assert_eq!(rig.party.submit_idea(&person("Ced"), input("three"), false).map(|_| ()), Ok(()));
    assert_eq!(rig.party.submit_idea(&person("Ced"), input("four"), false), Err(IdeaError::TooFast));
}

#[test]
fn the_recent_chat_is_attached_without_old_messages_or_system_lines() {
    let rig = rig();
    let who = crate::party::Actor { person: person("Ced"), admin: false };
    rig.party.post_message(&who, "old one").unwrap();
    rig.advance(46 * 60_000);
    rig.party.post_message(&who, "cannot write").unwrap();
    rig.party.say("joined", false, None, None).unwrap();
    rig.party
        .submit_idea(&person("Ced"), IdeaInput { device: Some("web".into()), ..input("help") }, false)
        .unwrap();
    let chat = &rig.party.ideas_for_triage(None)[0]["context"]["chat"];
    assert_eq!(chat.as_array().unwrap().len(), 1);
    assert_eq!(chat[0]["name"], "Ced");
    assert_eq!(chat[0]["text"], "cannot write");
}

#[test]
fn an_admin_updates_the_status_and_the_author_is_notified() {
    let rig = rig();
    let id = rig.party.submit_idea(&person("Cyril"), input("fix me"), false).unwrap();
    let before = rig.events("party.ideas").len();
    let idea = rig.party.update_idea(&id, Some("done"), Some("Fixed: thanks")).unwrap();
    assert_eq!((idea["status"].as_str(), idea["reply"].as_str()), (Some("done"), Some("Fixed: thanks")));
    assert_eq!(rig.events("party.ideas").len(), before + 1);
    assert_eq!(rig.events("party.ideas").last().unwrap()["ideas"][0]["status"], "done");
    let notified = rig.host.notifications();
    assert_eq!(notified.len(), 1);
    assert_eq!(rig.party.update_idea(&id, Some("nonsense"), None), Err(IdeaError::BadStatus));
    assert_eq!(rig.party.update_idea("ghost", Some("done"), None), Err(IdeaError::Unknown));
    assert_eq!(rig.party.ideas_for_triage(Some("received")).len(), 0);
    assert_eq!(rig.party.ideas_for_triage(Some("done")).len(), 1);
    rig.party.update_idea(&id, Some("inProgress"), Some("")).unwrap();
    assert!(rig.party.ideas_view()[0]["reply"].is_null());
}
