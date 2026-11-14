use serde_json::json;

use super::support::*;
use crate::sync::{Beat, FineDone};

fn beat(delay: f64) -> Beat {
    Beat { enabled: true, delay, ..Beat::default() }
}

#[test]
fn a_beat_starts_a_party_announced_in_the_chat_and_the_tick_broadcasts_its_state() {
    let rig = rig();
    rig.advance(200_000);
    rig.party.sync_beat(&person("Mamie"), beat(3.0));
    rig.party.sync_beat(&person("Papi"), beat(20.0));
    assert_eq!(rig.keys(), vec!["party.sync.started", "party.sync.joined"]);
    rig.party.tick();
    let event = rig.events("party.sync").pop().unwrap();
    let state = &event["state"];
    assert_eq!(state["target"], 12.0);
    assert_eq!(state["by"]["name"], "Papi");
    assert_eq!(rig.party.snapshot(&crate::party::Actor { person: person("Mamie"), admin: false })["sync"]["target"], 12.0);
}

#[test]
fn a_party_rebuilt_after_a_restart_announces_nothing() {
    let rig = rig();
    rig.party.sync_beat(&person("Mamie"), beat(3.0));
    assert!(rig.keys().is_empty());
}

#[test]
fn a_kick_needs_the_member_to_be_reachable_and_tells_them_in_an_addressed_event() {
    let rig = rig();
    rig.advance(200_000);
    rig.present(&["Mamie", "Papi"]);
    rig.party.sync_beat(&person("Mamie"), beat(3.0));
    assert_eq!(rig.party.kick("inconnu", ""), Err("notInParty"));
    assert_eq!(rig.party.kick("mamie", "annoying"), Ok(0));
    assert_eq!(rig.last_system().key.as_deref(), Some("party.sync.removedRequested"));
    let addressed: Vec<_> = rig.all_events().into_iter().filter(|(to, t, _)| to.as_deref() == Some("mamie") && t == "party.sync.kicked").collect();
    assert_eq!(addressed.len(), 1);
    assert!(addressed[0].2.get("seq").is_none());

    rig.party.sync_beat(&person("Sebi"), beat(9.0));
    assert_eq!(rig.party.kick("sebi", ""), Err("disconnected"));
    assert!(!rig.keys().iter().any(|k| k == "party.sync.removedDisruptions"));
}

#[test]
fn go_live_and_fine_sync_are_broadcast_and_report_to_the_chat() {
    let rig = rig();
    rig.advance(200_000);
    assert_eq!(rig.party.sync_go_live("agent"), Err("noActiveParty"));
    rig.party.sync_beat(&person("Mamie"), beat(3.0));
    rig.party.sync_beat(&person("Papi"), beat(5.0));
    rig.party.sync_fine_sync("agent").unwrap();
    assert_eq!(rig.events("party.sync.finesync").len(), 1);
    rig.party.sync_fine_done(&person("Mamie"), FineDone { ms: 40, seeks: 1, ..FineDone::default() });
    rig.party.sync_fine_done(&person("Papi"), FineDone { ms: 60, seeks: 2, ..FineDone::default() });
    let last = rig.last_system();
    assert_eq!(last.key.as_deref(), Some("party.sync.fineDone"));
    assert_eq!(last.params["items"].as_array().unwrap().len(), 2);
    assert_eq!(rig.party.sync_go_live("agent"), Ok(2));
    assert_eq!(rig.events("party.sync.golive").len(), 1);
    assert_eq!(rig.last_system().key.as_deref(), Some("party.sync.golive"));
}

#[test]
fn the_time_route_answers_the_server_clock() {
    let rig = rig();
    assert_eq!(rig.party.time_now(), T0);
    rig.advance(5);
    assert_eq!(rig.party.time_now(), T0 + 5);
}

#[test]
fn the_sync_state_event_pins_the_json_the_clients_read() {
    let rig = rig();
    rig.advance(200_000);
    rig.party.sync_beat(&person("Mamie"), beat(3.0));
    rig.party.tick();
    assert_eq!(
        rig.events("party.sync").last().unwrap()["state"],
        json!({ "target": 3.0, "by": { "userId": "mamie", "name": "Mamie" }, "capped": false,
                "members": [{ "userId": "mamie", "name": "Mamie" }], "active": 1 })
    );
}
