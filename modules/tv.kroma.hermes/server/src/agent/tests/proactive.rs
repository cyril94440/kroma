use serde_json::json;

use crate::testing::{programme, Rig};

fn off_air(rig: &Rig, viewers: serde_json::Value) -> serde_json::Value {
    let mut live = programme("beinsports1", "beIN Sports 1", "Foot : Lens - PSG", -30, 60);
    live.title = "Foot : Lens - PSG".into();
    let later = programme("m6", "M6", "Foot : Nantes - Brest", 120, 200);
    *rig.control.programmes.lock().unwrap() = vec![live, later];
    json!({ "state": "offAir", "channel": { "id": "canalfoot", "name": "Canal+ Foot" }, "viewers": viewers, "health": {} })
}

#[test]
fn an_off_air_channel_with_viewers_gets_alternatives_from_the_catalogue_without_the_model() {
    let rig = Rig::new();
    rig.agent.on_event("live.status", &off_air(&rig, json!([{ "userId": "u-mamie", "name": "Mamie" }])));
    let said = rig.room.texts();
    assert_eq!(said.len(), 1);
    assert!(said[0].starts_with("Canal+ Foot ne diffuse plus rien"), "{said:?}");
    assert!(said[0].contains("beIN Sports 1 (Foot : Lens - PSG)") && !said[0].contains("M6"), "{said:?}");
    assert!(rig.room.switches.lock().unwrap().is_empty());
    let decisions = rig.deps.store.audit_rows(Some("decisions"), None, 3).unwrap();
    assert_eq!(decisions[0].event, "offair_alternatives");
}

#[test]
fn nobody_watching_or_a_recent_alert_means_no_message() {
    let rig = Rig::new();
    rig.agent.on_event("live.status", &off_air(&rig, json!([])));
    assert!(rig.room.texts().is_empty());
    rig.agent.on_event("live.status", &off_air(&rig, json!([{ "userId": "u-mamie", "name": "Mamie" }])));
    rig.advance(60_000);
    rig.agent.on_event("live.status", &off_air(&rig, json!([{ "userId": "u-mamie", "name": "Mamie" }])));
    assert_eq!(rig.room.texts().len(), 1);
    rig.advance(900_000);
    rig.agent.on_event("live.status", &off_air(&rig, json!([{ "userId": "u-mamie", "name": "Mamie" }])));
    assert_eq!(rig.room.texts().len(), 2);
}

#[test]
fn a_running_channel_is_only_measured_and_failover_events_are_logged() {
    let rig = Rig::new();
    for kbps in [4000, 4100, 4200] {
        rig.agent.on_event("live.status", &json!({ "state": "running", "channel": { "id": "tf1", "name": "TF1" }, "health": { "sourceKbps": kbps } }));
    }
    rig.agent.on_event("live.failover", &json!({ "kind": "bothDown", "channel": "TF1" }));
    assert!(rig.room.texts().is_empty());
    let events: Vec<String> = rig.deps.store.audit_rows(Some("sources"), None, 5).unwrap().into_iter().map(|r| r.event).collect();
    assert_eq!(events, vec!["live_failover".to_string(), "live_status".to_string()], "a status that only moved its bitrate is not logged again");
}
