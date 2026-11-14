use serde_json::json;

use super::support::call;
use crate::peers::SwitchOutcome;
use crate::testing::Rig;

#[test]
fn list_channels_returns_refs_and_audits_the_raw_source() {
    let rig = Rig::new();
    let out = call(&rig, "list_channels", json!({ "query": "m6" }), "Mamie");
    assert_eq!(out["chaines"][0]["ref"], "m6");
    let rows = rig.deps.store.audit_rows(Some("sources"), None, 10).unwrap();
    assert_eq!(rows[0].event, "list_channels");
    assert_eq!(rows[0].data["top"][0]["id"], "m6");
}

#[test]
fn list_channels_needs_a_query_and_reports_a_catalogue_that_is_down() {
    let rig = Rig::new();
    assert_eq!(call(&rig, "list_channels", json!({}), "Mamie")["ok"], false);
}

#[test]
fn get_status_reports_what_is_on_and_who_watches() {
    let rig = Rig::new();
    let out = call(&rig, "get_status", json!({}), "Mamie");
    assert_eq!(out["chaine"]["nom"], "TF1");
    assert_eq!(out["en_ce_moment"]["titre"], "Journal");
    assert_eq!(out["spectateurs"], json!(["Mamie", "Papi"]));
    *rig.control.down.lock().unwrap() = true;
    let down = call(&rig, "get_status", json!({}), "Mamie");
    assert_eq!(down["ok"], false);
    assert!(down["erreur"].as_str().unwrap().contains("indisponible"));
}

#[test]
fn run_ops_diagnose_gives_a_verdict_from_the_measures() {
    let rig = Rig::new();
    rig.control.status.lock().unwrap().health.source_kbps = 4200.0;
    let stable = call(&rig, "run_ops", json!({ "action": "diagnose", "pseudo": "Papi" }), "Papi");
    assert!(stable["verdict"].as_str().unwrap().starts_with("serveur_stable"));
    rig.control.status.lock().unwrap().health.stutter = true;
    let unstable = call(&rig, "run_ops", json!({ "action": "diagnose" }), "Papi");
    assert!(unstable["verdict"].as_str().unwrap().starts_with("source_instable"));
    rig.control.status.lock().unwrap().health.black = true;
    assert!(call(&rig, "run_ops", json!({ "action": "diagnose" }), "Papi")["verdict"].as_str().unwrap().starts_with("image_noire"));
    rig.control.status.lock().unwrap().state = "offAir".into();
    assert!(call(&rig, "run_ops", json!({ "action": "diagnose" }), "Papi")["verdict"].as_str().unwrap().starts_with("chaine_hors_antenne"));
}

#[test]
fn run_ops_reads_flux_health_and_provider_and_refuses_unknown_actions() {
    let rig = Rig::new();
    assert_eq!(call(&rig, "run_ops", json!({ "action": "fournisseurs" }), "Papi")["actif"], "primary");
    assert_eq!(call(&rig, "run_ops", json!({ "action": "sante_flux" }), "Papi")["chaine"], "TF1");
    let refused = call(&rig, "run_ops", json!({ "action": "restart_ingest" }), "Papi");
    assert_eq!(refused["ok"], false);
}

#[test]
fn change_channel_asks_the_room_for_a_vote_in_the_authors_name() {
    let rig = Rig::new();
    let out = call(&rig, "change_channel", json!({ "query": "m6", "requester": "someone-else", "userId": "u-x" }), "Mamie");
    assert_eq!(out["status"], "vote_open");
    let switches = rig.room.switches.lock().unwrap().clone();
    assert_eq!(switches, vec![("m6".to_string(), "u-mamie".to_string(), "m6".to_string())]);
}

#[test]
fn a_direct_switch_is_announced_by_the_system() {
    let rig = Rig::new();
    *rig.room.switch_reply.lock().unwrap() = Some(SwitchOutcome { outcome: "switchedDirectly".into(), ..SwitchOutcome::default() });
    assert_eq!(call(&rig, "change_channel", json!({ "query": "arte" }), "Mamie")["status"], "switched");
}

#[test]
fn refusals_come_back_to_the_model_with_their_reason() {
    let rig = Rig::new();
    for (reason, status) in [("alreadyOn", "already_on"), ("busy", "busy"), ("presenceUnavailable", "presence_indisponible"), ("offAir", "refused")] {
        *rig.room.switch_reply.lock().unwrap() = Some(SwitchOutcome { outcome: "refused".into(), reason: Some(reason.into()), vote_id: None });
        let out = call(&rig, "change_channel", json!({ "query": "m6" }), "Mamie");
        assert_eq!(out["ok"], false, "{reason}");
        assert_eq!(out["status"], status);
        assert!(out["raison"].as_str().unwrap().len() > 3);
    }
}

#[test]
fn an_unknown_channel_is_not_requested_and_suggests_nothing_invented() {
    let rig = Rig::new();
    let out = call(&rig, "change_channel", json!({ "query": "chaine inconnue" }), "Mamie");
    assert_eq!(out["status"], "not_found");
    assert!(rig.room.switches.lock().unwrap().is_empty());
    let decisions = rig.deps.store.audit_rows(Some("decisions"), None, 5).unwrap();
    assert_eq!(decisions[0].event, "change_channel_refused");
}

#[test]
fn a_reference_must_come_from_the_catalogue() {
    let rig = Rig::new();
    assert_eq!(call(&rig, "change_channel", json!({ "ref": "ghost" }), "Mamie")["status"], "ref_inconnue");
    assert_eq!(call(&rig, "change_channel", json!({ "ref": "arte" }), "Mamie")["status"], "vote_open");
    assert_eq!(rig.room.switches.lock().unwrap().len(), 1);
    assert_eq!(call(&rig, "change_channel", json!({}), "Mamie")["ok"], false);
}
