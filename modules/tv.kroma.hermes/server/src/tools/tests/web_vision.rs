use serde_json::json;

use super::support::run;
use crate::model::{ModelError, ScreenReading, WebAnswer};
use crate::peers::Snapshot;
use crate::testing::{Rig, Scripted};

fn answer(text: &str) -> Option<Result<WebAnswer, ModelError>> {
    Some(Ok(WebAnswer { text: text.into(), sources: vec!["lequipe.fr".into()] }))
}

#[test]
fn an_undated_answer_is_cached_for_ten_minutes_with_its_sources() {
    let rig = Rig::new();
    let model = Scripted::new(Vec::new());
    *model.0.search.lock().unwrap() = answer("Le hors-jeu sanctionne une position.");
    let (first, _) = run(&rig, &model, "web_search", json!({ "query": "c'est quoi le hors-jeu" }), "Mamie");
    assert_eq!(first["sources"], json!(["lequipe.fr"]));
    run(&rig, &model, "web_search", json!({ "query": "C'est quoi le hors jeu ?" }), "Mamie");
    assert_eq!(model.0.searches.lock().unwrap().len(), 1);
    rig.advance(601_000);
    run(&rig, &model, "web_search", json!({ "query": "c'est quoi le hors-jeu" }), "Mamie");
    assert_eq!(model.0.searches.lock().unwrap().len(), 2);
}

#[test]
fn a_score_question_is_never_served_from_the_cache() {
    let rig = Rig::new();
    let model = Scripted::new(Vec::new());
    *model.0.search.lock().unwrap() = answer("Lens mène 1-0 à la mi-temps, ce soir.");
    run(&rig, &model, "web_search", json!({ "query": "quel est le score de Lens" }), "Mamie");
    run(&rig, &model, "web_search", json!({ "query": "quel est le score de Lens" }), "Mamie");
    assert_eq!(model.0.searches.lock().unwrap().len(), 2);
}

#[test]
fn a_dated_question_whose_answer_has_no_date_mark_is_refused() {
    let rig = Rig::new();
    let model = Scripted::new(Vec::new());
    *model.0.search.lock().unwrap() = answer("Lens a battu Paris 2-1.");
    let (out, _) = run(&rig, &model, "web_search", json!({ "query": "quel est le score de Lens" }), "Mamie");
    assert_eq!(out["non_verifie"], true);
    let decisions = rig.deps.store.audit_rows(Some("decisions"), None, 3).unwrap();
    assert_eq!(decisions[0].event, "web_search_unverified");
}

#[test]
fn a_failed_search_is_returned_not_hidden() {
    let rig = Rig::new();
    let model = Scripted::new(Vec::new());
    *model.0.search.lock().unwrap() = Some(Err(ModelError::Transient("boom".into())));
    let (out, _) = run(&rig, &model, "web_search", json!({ "query": "météo" }), "Mamie");
    assert_eq!(out["ok"], false);
    assert!(out["erreur"].as_str().unwrap().contains("boom"));
}

fn snapshot(rig: &Rig) {
    *rig.control.snapshot.lock().unwrap() = Some(Ok(Snapshot { jpeg_base64: "QUJD".into(), at: rig.now() - 3000 }));
}

#[test]
fn look_at_live_reads_the_captured_frame_and_reports_trust() {
    let rig = Rig::new();
    snapshot(&rig);
    let model = Scripted::new(Vec::new());
    let mut reading = ScreenReading::unknown();
    reading.kind = "direct".into();
    reading.team1 = "Lens".into();
    reading.confidence = 0.9;
    *model.0.screen.lock().unwrap() = Some(Ok(reading));
    let (out, _) = run(&rig, &model, "look_at_live", json!({ "question": "qui joue ?" }), "Mamie");
    assert_eq!((out["type"].as_str(), out["team1"].as_str(), out["fiable"].as_bool()), (Some("direct"), Some("Lens"), Some(true)));
    assert_eq!(out["capture_il_y_a_s"], 3);
    assert_eq!(rig.deps.store.audit_rows(Some("vision"), None, 3).unwrap()[0].event, "reading");
}

#[test]
fn a_live_module_without_snapshot_switches_the_tool_off_for_ten_minutes() {
    let rig = Rig::new();
    let (out, _) = run(&rig, &Scripted::new(Vec::new()), "look_at_live", json!({}), "Mamie");
    assert_eq!(out["ok"], false);
    assert!(!rig.deps.snapshot_supported(rig.now()));
    rig.advance(601_000);
    assert!(rig.deps.snapshot_supported(rig.now()));
}

#[test]
fn an_empty_or_huge_capture_is_rejected_before_any_model_call() {
    let rig = Rig::new();
    *rig.control.snapshot.lock().unwrap() = Some(Ok(Snapshot { jpeg_base64: String::new(), at: 0 }));
    assert_eq!(run(&rig, &Scripted::new(Vec::new()), "look_at_live", json!({}), "Mamie").0["ok"], false);
    *rig.control.snapshot.lock().unwrap() = Some(Ok(Snapshot { jpeg_base64: "A".repeat(2_000_001), at: 0 }));
    assert_eq!(run(&rig, &Scripted::new(Vec::new()), "look_at_live", json!({}), "Mamie").0["ok"], false);
}
