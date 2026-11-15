use serde_json::json;

use super::support::{call, fail, history, point, reply};
use crate::peers::{HistoryPoint, Restart};
use crate::testing::Rig;
use crate::tools::{Availability, Kind, ToolSet};

fn run(rig: &Rig, action: &str) -> serde_json::Value {
    call(rig, "run_ops", json!({ "action": action, "pseudo": "Mamie" }), "Mamie")
}

fn steady(rig: &Rig) {
    rig.control.status.lock().unwrap().health.source_kbps = 4200.0;
}

#[test]
fn recent_restarts_in_the_history_make_the_source_unstable_and_point_to_the_backup() {
    let rig = Rig::new();
    steady(&rig);
    let mut data: Vec<HistoryPoint> = (0..60).map(|i| point(i, 4000.0)).collect();
    data[40].restarts = 4;
    for p in &mut data[41..] {
        p.restarts = 5;
    }
    reply(&rig.control.history, history(data));

    let out = run(&rig, "diagnose");

    let verdict = out["verdict"].as_str().unwrap();
    assert!(verdict.starts_with("source_instable") && verdict.contains("SECOURS"), "{verdict}");
    assert_eq!(out["historique"]["relances_10_min"], 2);
}

#[test]
fn a_diagnosis_lists_the_outage_episodes_of_the_last_half_hour() {
    let rig = Rig::new();
    steady(&rig);
    reply(&rig.control.history, history((0..60).map(|i| point(i, if (20..24).contains(&i) { 0.0 } else { 4000.0 })).collect()));

    let out = run(&rig, "diagnose");

    assert_eq!((out["historique"]["episodes_30_min"][0]["duree_s"].clone(), out["verdict"].as_str().map(|v| v.starts_with("serveur_stable"))), (json!(40), Some(true)));
}

#[test]
fn a_stable_server_with_freezes_on_the_complainers_app_points_at_their_side() {
    let rig = Rig::new();
    steady(&rig);
    *rig.room.logs_reply.lock().unwrap() = Some(Ok(vec![json!({ "event": "relance_gel" }), json!({ "event": "relance_gel" }), json!({ "event": "repris" })]));

    let out = run(&rig, "diagnose");

    assert!(out["verdict"].as_str().unwrap().starts_with("cote_spectateur"));
    assert_eq!((out["spectateur"]["pseudo"].clone(), out["spectateur"]["gels_30_min"].clone()), (json!("Mamie"), json!(2)));
    assert_eq!(rig.room.logs_asks.lock().unwrap()[0], ("u-mamie".to_string(), Some("u-mamie".to_string()), 200, Some(30)));
}

#[test]
fn a_diagnosis_still_works_with_a_live_module_that_has_no_history() {
    let rig = Rig::new();
    steady(&rig);

    let out = run(&rig, "diagnose");

    assert!(out["verdict"].as_str().unwrap().starts_with("serveur_stable"));
    assert!(out.get("historique").is_none());
}

#[test]
fn the_flux_health_carries_the_measures_and_the_latest_history_points() {
    let rig = Rig::new();
    reply(&rig.control.history, history((0..10).map(|i| point(i, 4000.0)).collect()));

    let out = run(&rig, "sante_flux");

    assert_eq!((out["chaine"].clone(), out["detail_saccades"].as_array().unwrap().len()), (json!("TF1"), 6));
}

#[test]
fn the_providers_report_names_the_active_one_and_the_latest_switches() {
    let rig = Rig::new();
    let mut recorded = history(Vec::new());
    recorded.provider_events = vec![json!({ "kind": "failover", "from": "primary", "to": "backup", "channel": "TF1", "detail": "silence", "at": 1_794_000_000_000_i64 }), json!({ "kind": "coverageGap", "channel": "Arte" })];
    reply(&rig.control.history, recorded);

    let out = run(&rig, "fournisseurs");

    assert_eq!(out["actif"], "principal");
    assert_eq!((out["evenements_recents"][0]["genre"].clone(), out["evenements_recents"][1]["vers"].clone()), (json!("coverageGap"), json!("backup")));
}

#[test]
fn a_restart_is_asked_in_the_authors_name_and_succeeds_with_the_channel_name() {
    let rig = Rig::new();
    reply(&rig.control.restart, Restart { restarted: true, error: None });

    let out = run(&rig, "restart_ingest");

    assert_eq!((out["ok"].clone(), out["chaine"].clone()), (json!(true), json!("TF1")));
    assert!(out["message"].as_str().unwrap().contains("~5 s"));
    assert_eq!(*rig.control.calls.lock().unwrap(), vec![("restart".to_string(), "u-mamie".to_string())]);
}

#[test]
fn every_refusal_of_the_restart_point_returns_its_reason_to_the_model() {
    for (code, raison) in [("noChannel", "aucune_chaine"), ("standby", "veille"), ("streamHealthy", "flux_sain"), ("recentRestart", "relance_recente"), ("channelChanged", "chaine_changee")] {
        let rig = Rig::new();
        reply(&rig.control.restart, Restart { restarted: false, error: Some(code.into()) });

        let out = run(&rig, "restart_ingest");

        assert_eq!((out["ok"].clone(), out["raison"].as_str()), (json!(false), Some(raison)), "{code}");
        assert!(out["message"].as_str().unwrap().len() > 10, "{code}");
    }
}

#[test]
fn a_live_module_without_restart_answers_with_a_failure_not_a_crash() {
    let rig = Rig::new();
    fail(&rig.control.restart, "HTTP 404: not found");

    let out = run(&rig, "restart_ingest");

    assert_eq!(out["ok"], false);
    assert!(out["erreur"].as_str().unwrap().contains("relance impossible"));
}

#[test]
fn a_restart_is_a_refusable_action_while_the_other_actions_stay_investigations() {
    let rig = Rig::new();
    let set = ToolSet::build(&rig.deps, &Availability::default(), rig.now());
    let ops = set.get("run_ops").unwrap();

    assert_eq!(ops.kind_for(&json!({ "action": "restart_ingest" })), Kind::Refusable);
    assert_eq!(ops.kind_for(&json!({ "action": "diagnose" })), Kind::Investigation);
}
