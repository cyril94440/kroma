use serde_json::json;

use super::support::call;
use crate::testing::Rig;

fn entry(at: i64, name: &str, category: &str, event: &str) -> serde_json::Value {
    json!({ "at": at, "userId": format!("u-{}", name.to_lowercase()), "name": name, "device": "iPhone", "category": category, "event": event, "data": { "ms": 800 } })
}

#[test]
fn go_live_names_the_author_and_leaves_the_announcement_to_the_party() {
    let rig = Rig::new();
    *rig.room.go_live_reply.lock().unwrap() = Some(Ok(json!({ "members": 2 })));

    let out = call(&rig, "party_go_live", json!({}), "Mamie");

    assert_eq!((out["ok"].clone(), out["membres"].clone()), (json!(true), json!(2)));
    assert_eq!(*rig.room.party_calls.lock().unwrap(), vec![("goLive".to_string(), "u-mamie".to_string())]);
}

#[test]
fn fine_sync_and_go_live_refusals_are_returned_with_their_reason() {
    let rig = Rig::new();
    *rig.room.go_live_reply.lock().unwrap() = Some(Err("noActiveParty".into()));
    *rig.room.fine_sync_reply.lock().unwrap() = Some(Err("needTwoMembers".into()));

    let go = call(&rig, "party_go_live", json!({}), "Mamie");
    let fine = call(&rig, "party_fine_sync", json!({}), "Mamie");

    assert_eq!((go["ok"].clone(), go["raison"].as_str()), (json!(false), Some("pas_de_watch_party")));
    assert_eq!((fine["ok"].clone(), fine["raison"].as_str()), (json!(false), Some("deux_membres_requis")));
}

#[test]
fn a_party_without_the_new_methods_loses_the_tools_for_ten_minutes() {
    let rig = Rig::new();

    let out = call(&rig, "party_fine_sync", json!({}), "Mamie");

    assert_eq!(out["ok"], false);
    assert!(!rig.deps.supported("fineSync", rig.now()));
    assert!(rig.deps.supported("goLive", rig.now()));
}

#[test]
fn client_logs_are_filtered_by_person_and_category_and_summarised_per_viewer() {
    let rig = Rig::new();
    *rig.room.logs_reply.lock().unwrap() = Some(Ok(vec![entry(1, "Mamie", "player", "relance_gel"), entry(2, "Mamie", "player", "relance_gel"), entry(3, "Mamie", "ws", "reconnexion"), entry(4, "Papi", "player", "repris")]));

    let out = call(&rig, "get_client_logs", json!({ "minutes": 90, "pseudo": "mamie", "categorie": "player" }), "Papi");

    assert_eq!((out["total"].clone(), out["par_spectateur"][0]["pseudo"].clone(), out["par_spectateur"][0]["evenements"].clone()), (json!(2), json!("Mamie"), json!(2)));
    assert_eq!(out["par_spectateur"][0]["detail"][0], json!({ "evenement": "player/relance_gel", "nombre": 2 }));
    assert_eq!(out["evenements"].as_array().unwrap().len(), 2);
    assert_eq!(*rig.room.logs_asks.lock().unwrap(), vec![("u-papi".to_string(), None, 300, Some(90))]);
}

#[test]
fn the_raw_events_are_capped_and_the_window_is_bounded_to_three_days() {
    let rig = Rig::new();
    *rig.room.logs_reply.lock().unwrap() = Some(Ok((0..250).map(|i| entry(i, "Mamie", "player", "gel")).collect()));

    let out = call(&rig, "get_client_logs", json!({ "minutes": 99_999, "limit": 5 }), "Papi");

    assert_eq!((out["total"].clone(), out["evenements"].as_array().unwrap().len()), (json!(250), 5));
    assert_eq!(rig.room.logs_asks.lock().unwrap()[0].3, Some(4320));
}

#[test]
fn a_party_that_cannot_give_logs_is_reported_not_hidden() {
    let rig = Rig::new();

    let out = call(&rig, "get_client_logs", json!({}), "Papi");

    assert_eq!(out["ok"], false);
    assert!(!rig.deps.supported("clientLogs", rig.now()));
}

#[test]
fn a_non_administrator_is_refused_the_party_actions_with_the_servers_reason() {
    let rig = Rig::new();
    *rig.room.go_live_reply.lock().unwrap() = Some(Err("adminOnly".into()));
    *rig.room.fine_sync_reply.lock().unwrap() = Some(Err("adminOnly".into()));

    for tool in ["party_go_live", "party_fine_sync"] {
        let out = call(&rig, tool, json!({}), "Mamie");
        assert_eq!((out["ok"].clone(), out["raison"].as_str()), (json!(false), Some("reserve_aux_admins")), "{tool}");
    }
}

#[test]
fn client_logs_fall_back_to_the_authors_own_journal_when_the_party_refuses_the_whole_room() {
    let rig = Rig::new();
    *rig.room.logs_reply.lock().unwrap() = Some(Err("adminOnly".into()));
    *rig.room.own_logs_reply.lock().unwrap() = Some(vec![entry(1, "Mamie", "player", "gel")]);

    let out = call(&rig, "get_client_logs", json!({}), "Mamie");

    assert_eq!(out["total"], 1);
    assert!(out["portee"].as_str().unwrap().contains("administrateurs"));
    assert_eq!(
        *rig.room.logs_asks.lock().unwrap(),
        vec![("u-mamie".to_string(), None, 300, Some(30)), ("u-mamie".to_string(), Some("u-mamie".to_string()), 300, Some(30))]
    );
}
