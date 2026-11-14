use serde_json::json;

use super::support::*;
use crate::chat_rules::REACTION_EMOJIS;
use crate::party::{Actor, ChatError};

fn actor(name: &str) -> Actor {
    Actor { person: person(name), admin: false }
}

#[test]
fn a_message_is_stored_clipped_published_and_flagged_for_the_assistant() {
    let rig = rig();
    let long = "x".repeat(900);
    let m = rig.party.post_message(&actor("Mamie"), &format!("  Hermès, {long}")).unwrap();
    assert_eq!(m.text.chars().count(), 500);
    let events = rig.events("party.message");
    let e = events.last().unwrap();
    assert_eq!(e["id"], json!(m.id));
    assert_eq!(e["userId"], "mamie");
    assert_eq!(e["name"], "Mamie");
    assert_eq!(e["kind"], "user");
    assert_eq!(e["mentionsAgent"], true);
    assert_eq!(e["isQuestion"], false);
    assert!(e["at"].as_i64().unwrap() > 0);
    assert_eq!(rig.party.history(10, None)[0].id, m.id);
    assert_eq!(rig.party.post_message(&actor("Mamie"), "   "), Err(ChatError::Empty));
}

#[test]
fn the_rate_limit_is_eight_messages_in_eight_seconds_per_person() {
    let rig = rig();
    for i in 0..8 {
        assert!(rig.party.post_message(&actor("Mamie"), &format!("m{i}")).is_ok());
    }
    assert_eq!(rig.party.post_message(&actor("Mamie"), "nine"), Err(ChatError::RateLimited));
    assert!(rig.party.post_message(&actor("Papi"), "me").is_ok());
    rig.advance(8_001);
    assert!(rig.party.post_message(&actor("Mamie"), "again").is_ok());
}

#[test]
fn the_agent_and_the_system_speak_through_say_and_the_message_is_pinned() {
    let rig = rig();
    let agent = rig.party.say("Bonsoir", true, Some(json!({ "type": "table" })), Some("m1".into())).unwrap();
    let system = rig.party.say("Maintenance", false, None, None).unwrap();
    let history = rig.party.room_history(10, None);
    assert_eq!(history.len(), 2);
    assert_eq!((history[0].id.as_str(), history[0].kind.as_str(), history[0].name.as_str()), (agent.as_str(), "agent", "Hermès"));
    assert_eq!((history[1].id.as_str(), history[1].kind.as_str()), (system.as_str(), "system"));
    assert_eq!(
        serde_json::to_value(&history[0]).unwrap(),
        json!({ "id": agent, "userId": null, "name": "Hermès", "kind": "agent", "text": "Bonsoir", "at": history[0].at })
    );
    let stored = rig.party.history(10, None);
    assert_eq!(stored[0].card, Some(json!({ "type": "table" })));
    assert_eq!(stored[0].reply_to.as_deref(), Some("m1"));
    assert!(rig.events("party.message").iter().all(|e| e.get("mentionsAgent").is_none()));
}

#[test]
fn the_history_pages_backwards_with_before() {
    let rig = rig();
    for i in 0..5 {
        rig.party.say(&format!("m{i}"), false, None, None).unwrap();
        rig.advance(10);
    }
    let last = rig.party.room_history(2, None);
    assert_eq!(last.iter().map(|m| m.text.as_str()).collect::<Vec<_>>(), vec!["m3", "m4"]);
    let older = rig.party.room_history(2, Some(last[0].at));
    assert_eq!(older.iter().map(|m| m.text.as_str()).collect::<Vec<_>>(), vec!["m1", "m2"]);
}

#[test]
fn a_reaction_toggles_per_person_and_only_the_allowed_ones_count() {
    let rig = rig();
    let m = rig.party.post_message(&actor("Mamie"), "but !").unwrap();
    let goal = REACTION_EMOJIS[0];
    rig.party.toggle_reaction(&actor("Cyril"), &m.id, goal).unwrap();
    let stored = rig.party.history(10, None).remove(0);
    assert_eq!(stored.reactions[goal][0].name, "Cyril");
    assert_eq!(rig.events("party.reactions").last().unwrap()["id"], json!(m.id));
    rig.party.toggle_reaction(&actor("Papi"), &m.id, goal).unwrap();
    assert_eq!(rig.party.history(10, None).remove(0).reactions[goal].len(), 2);
    rig.party.toggle_reaction(&actor("Cyril"), &m.id, goal).unwrap();
    rig.party.toggle_reaction(&actor("Papi"), &m.id, goal).unwrap();
    assert!(rig.party.history(10, None).remove(0).reactions.is_empty());
    let before = rig.events("party.reactions").len();
    assert_eq!(rig.party.toggle_reaction(&actor("Cyril"), &m.id, "\u{1F4A9}"), Err(ChatError::Unknown));
    assert_eq!(rig.party.toggle_reaction(&actor("Cyril"), "nope", goal), Err(ChatError::Unknown));
    assert_eq!(rig.party.toggle_reaction(&actor("Cyril"), "", goal), Err(ChatError::Unknown));
    assert_eq!(rig.events("party.reactions").len(), before);
}

#[test]
fn reactions_only_reach_the_two_hundred_latest_messages() {
    let rig = rig();
    let first = rig.party.say("old", false, None, None).unwrap();
    for i in 0..200 {
        rig.party.say(&format!("m{i}"), false, None, None).unwrap();
    }
    assert_eq!(rig.party.toggle_reaction(&actor("Cyril"), &first, REACTION_EMOJIS[0]), Err(ChatError::Unknown));
}

#[test]
fn a_floating_reaction_is_relayed_and_rate_limited_to_eight_per_three_seconds() {
    let rig = rig();
    for _ in 0..8 {
        rig.party.floating_reaction(&actor("Cyril"), "\u{1F525}").unwrap();
    }
    assert_eq!(rig.party.floating_reaction(&actor("Cyril"), "\u{1F525}"), Err(ChatError::RateLimited));
    assert_eq!(rig.party.floating_reaction(&actor("Cyril"), "  "), Err(ChatError::Empty));
    let e = rig.events("party.reaction");
    assert_eq!(e.len(), 8);
    assert_eq!(e[0]["emoji"], "\u{1F525}");
    assert_eq!(e[0]["name"], "Cyril");
    rig.party.floating_reaction(&actor("Papi"), &"a".repeat(30)).unwrap();
    assert_eq!(rig.events("party.reaction").last().unwrap()["emoji"].as_str().unwrap().chars().count(), 8);
}

#[test]
fn the_activity_line_is_set_cleared_and_expires_on_its_own() {
    let rig = rig();
    rig.party.set_activity(Some("Searching the web"));
    assert_eq!(rig.events("party.activity").last().unwrap()["label"], "Searching the web");
    assert_eq!(rig.party.snapshot(&actor("Mamie"))["activity"], "Searching the web");
    rig.party.set_activity(None);
    assert!(rig.events("party.activity").last().unwrap()["label"].is_null());
    rig.party.set_activity(Some("Looking"));
    rig.advance(121_000);
    rig.party.tick();
    assert!(rig.events("party.activity").last().unwrap()["label"].is_null());
    assert!(rig.party.snapshot(&actor("Mamie"))["activity"].is_null());
}

#[test]
fn events_carry_a_strictly_increasing_sequence_and_the_state_converges_after_a_gap() {
    let rig = rig();
    let me = actor("Mamie");
    rig.party.post_message(&me, "one").unwrap();
    let seen = rig.party.snapshot(&me)["seq"].as_u64().unwrap();
    rig.party.post_message(&me, "two").unwrap();
    rig.party.post_message(&me, "three").unwrap();
    let seqs: Vec<u64> = rig.all_events().iter().filter(|(to, _, _)| to.is_none())
        .map(|(_, _, p)| p["seq"].as_u64().unwrap()).collect();
    assert!(seqs.windows(2).all(|w| w[1] == w[0] + 1), "{seqs:?}");
    assert_eq!(seqs[0] + 2, *seqs.last().unwrap());
    let missed_after = seqs.iter().filter(|s| **s > seen).count();
    assert_eq!(missed_after, 2);
    let state = rig.party.snapshot(&me);
    assert_eq!(state["seq"].as_u64().unwrap(), *seqs.last().unwrap());
    let texts: Vec<&str> = state["messages"].as_array().unwrap().iter().map(|m| m["text"].as_str().unwrap()).collect();
    assert_eq!(texts, vec!["one", "two", "three"]);
}

#[test]
fn the_sequence_never_goes_back_after_a_restart() {
    let rig = rig();
    let me = actor("Mamie");
    rig.party.post_message(&me, "x").unwrap();
    let last = rig.party.snapshot(&me)["seq"].as_u64().unwrap();
    let pool = kroma_module_sdk::host::HostStorage::store(&*rig.host).clone();
    let host: std::sync::Arc<dyn kroma_module_sdk::host::HostCtx> = rig.host.clone();
    let again = crate::party::Party::with_clock(host, pool, rig.live.clone(), std::sync::Arc::new(|| 5));
    assert!(again.snapshot(&me)["seq"].as_u64().unwrap() >= last);
}

#[test]
fn the_state_pins_the_json_a_joining_client_reads() {
    let rig = rig();
    rig.present(&["Mamie"]);
    let state = rig.party.snapshot(&actor("Mamie"));
    let mut keys: Vec<&str> = state.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec!["activity", "config", "lastPoll", "me", "messages", "now", "poll", "presence", "seq", "stats", "sync", "vote"]
    );
    assert_eq!(state["me"], json!({ "userId": "mamie", "name": "Mamie", "admin": false }));
    assert_eq!(state["config"], json!({ "voteSeconds": 30, "agentName": "Hermès" }));
    assert_eq!(state["presence"][0]["userId"], "mamie");
}

#[test]
fn old_messages_are_archived_by_the_job() {
    let rig = rig();
    rig.party.say("old", false, None, None).unwrap();
    rig.advance(31 * 86_400_000);
    rig.party.say("new", false, None, None).unwrap();
    assert_eq!(rig.party.archive().unwrap(), 1);
    assert_eq!(rig.texts(), vec!["new"]);
}

#[test]
fn a_joining_client_only_gets_the_messages_of_the_last_day() {
    let rig = rig();
    rig.party.say("yesterday", false, None, None).unwrap();
    rig.advance(25 * 3_600_000);
    rig.party.say("today", false, None, None).unwrap();
    let state = rig.party.snapshot(&actor("Mamie"));
    assert_eq!(state["messages"].as_array().unwrap().len(), 1);
    assert_eq!(rig.party.history(10, None).len(), 2);
}
