use serde_json::json;

use super::loops::{calls, play, result_of};
use super::scans::{direct, frames, item, live_picture, readings, rig_with_backup, state};
use crate::peers::{BenchmarkStart, Candidate};
use crate::testing::Scripted;

fn bench(args: serde_json::Value) -> (&'static str, serde_json::Value) {
    ("benchmark_channels", args)
}

fn start(rig: &crate::testing::Rig, status: &str, reason: Option<&str>) {
    *rig.control.benchmark.lock().unwrap() = Some(Ok(BenchmarkStart { status: status.into(), id: None, reason: reason.map(str::to_string), total: 2, already_done: false }));
}

#[test]
fn every_refusal_of_the_benchmark_point_comes_back_with_a_status_the_model_can_use() {
    for (reason, key, value) in [("scanRunning", "status", "occupe"), ("mixedProviders", "error", ""), ("unknownChannel", "error", ""), ("noChannel", "error", ""), ("other", "status", "refus")] {
        let rig = rig_with_backup();
        start(&rig, "refused", Some(reason));

        let model = play(&rig, "scanne", vec![calls(vec![bench(json!({ "query": "ligue1+", "fournisseur": "secours" }))]), calls(vec![("reply", json!({ "message": "ok" }))])]);

        let out = result_of(&model, 0, 0);
        assert!(out[key].is_string() && (value.is_empty() || out[key] == value), "{reason}: {out}");
    }
}

#[test]
fn a_scan_asks_for_a_family_or_references_and_refuses_unknown_ones() {
    let rig = rig_with_backup();

    let model = play(&rig, "scanne", vec![calls(vec![bench(json!({})), bench(json!({ "refs": ["#7.1"] })), bench(json!({ "query": "rien du tout", "fournisseur": "secours" }))]), calls(vec![("reply", json!({ "message": "ok" }))])]);

    assert!(result_of(&model, 0, 0)["error"].as_str().unwrap().contains("famille"));
    assert!(rig.control.benchmark_ids.lock().unwrap().is_empty());
}

#[test]
fn references_of_two_providers_are_refused_before_any_scan_starts() {
    let rig = rig_with_backup();
    rig.control.channels.lock().unwrap().push(Candidate { id: "p:1".into(), name: "TF1".into(), group: "FR".into(), score: 5.0, now: None, provider: "primary".into(), event: None });

    let model = play(&rig, "scanne", vec![calls(vec![bench(json!({ "refs": ["s:1", "p:1"] }))]), calls(vec![("reply", json!({ "message": "ok" }))])]);

    assert!(result_of(&model, 0, 0)["error"].as_str().unwrap().contains("deux fournisseurs"));
    assert!(rig.control.benchmark_ids.lock().unwrap().is_empty());
}

#[test]
fn at_most_eight_channels_are_scanned_whatever_the_limit_asked() {
    let rig = rig_with_backup();
    for n in 5..=15 {
        rig.control.channels.lock().unwrap().push(super::scans::backup(&format!("s:{n}"), &format!("|FR| LIGUE1+ {n}")));
    }
    *rig.control.scans.lock().unwrap() = vec![state(false, vec![item(1, "s:1", true, None)])];

    play(&rig, "scanne", vec![calls(vec![bench(json!({ "query": "ligue1+", "fournisseur": "secours", "limit": 24 }))]), calls(vec![("reply", json!({ "message": "ok" }))])]);

    assert_eq!(rig.control.benchmark_ids.lock().unwrap()[0].len(), 8);
}

#[test]
fn a_scan_that_overruns_its_budget_returns_what_it_has_flagged_as_cut_short() {
    let rig = rig_with_backup();
    *rig.control.scans.lock().unwrap() = vec![state(true, vec![item(1, "s:1", true, None)])];
    let model = Scripted::new(vec![calls(vec![bench(json!({ "query": "ligue1+", "fournisseur": "secours" }))]), calls(vec![("reply", json!({ "message": "ok" }))])]);
    *model.0.per_step.lock().unwrap() = Some((rig.clock.clone(), 100_000));
    rig.use_model(std::sync::Arc::new(model.clone()));

    rig.hear("m-1", "Mamie", "scanne");
    for _ in 0..40 {
        rig.agent.tick();
        rig.advance(30_000);
    }

    let out = result_of(&model, 0, 0);
    assert!(out["delai_depasse"].as_str().unwrap().contains("écourté"));
}

#[test]
fn a_live_module_without_frame_access_still_returns_the_capture_states_and_says_why_there_is_no_verdict() {
    let rig = rig_with_backup();
    *rig.control.scans.lock().unwrap() = vec![state(false, vec![item(1, "s:1", false, None), item(2, "s:2", true, None)])];

    let model = play(&rig, "scanne", vec![calls(vec![bench(json!({ "query": "ligue1+", "fournisseur": "secours", "comparer_au_direct": true }))]), calls(vec![("reply", json!({ "message": "ok" }))])]);

    let out = result_of(&model, 0, 0);
    assert!(out["lecture_image"].as_str().unwrap().contains("scanFrame"));
    assert_eq!(out["groupes"][0]["contenu"], "image non analysée");
    assert!(out["groupes"][0].get("meme_que_direct").is_none() && out.get("aucun_direct").is_none());
    assert!(!rig.deps.supported("scanFrame", rig.now()));
}

#[test]
fn portal_errors_on_every_channel_are_reported_as_the_providers_refusal_not_dead_channels() {
    let rig = rig_with_backup();
    *rig.control.scans.lock().unwrap() = vec![state(false, vec![item(1, "s:1", false, Some("HTTP 503")), item(2, "s:2", false, Some("HTTP 503")), item(3, "s:3", false, Some("HTTP 502"))])];

    let model = play(&rig, "scanne", vec![calls(vec![bench(json!({ "query": "ligue1+", "fournisseur": "secours" }))]), calls(vec![("reply", json!({ "message": "ok" }))])]);

    assert!(result_of(&model, 0, 0)["avertissement"].as_str().unwrap().contains("ne les sert pas en ce moment"));
}

#[test]
fn a_search_text_flags_the_channels_that_show_the_searched_match_even_when_the_live_is_black() {
    let rig = rig_with_backup();
    *rig.control.scans.lock().unwrap() = vec![state(false, vec![item(1, "s:1", false, None), item(2, "s:2", false, None)])];
    frames(&rig, &[1, 2]);
    let model = Scripted::new(vec![calls(vec![bench(json!({ "query": "ligue1+", "fournisseur": "secours", "cherche": "Lens - Paris FC" }))]), calls(vec![("reply", json!({ "message": "ok" }))])]);
    readings(&model, vec![direct("Lyon", "Monaco", "0-0", "10:00"), direct("Lens", "Paris FC", "1-0", "34:12")]);
    rig.use_model(std::sync::Arc::new(model.clone()));

    rig.hear("m-1", "Mamie", "où est Lens - Paris FC ?");
    rig.drain();

    let out = result_of(&model, 0, 0);
    assert_eq!((out["groupes"][0]["correspond_a_la_recherche"].as_str(), out["groupes"][0]["chaines"][0]["ref"].as_str()), (Some("oui"), Some("s:2")));
    assert_eq!(out["groupes"][1]["correspond_a_la_recherche"], "non");
}

#[test]
fn the_reference_falls_back_on_the_last_reading_remembered_when_the_live_picture_has_no_teams() {
    let rig = rig_with_backup();
    live_picture(&rig);
    rig.deps.live_seen.set("tf1", &direct("Lens", "Paris FC", "1-0", "30:00"), rig.now() - 60_000);
    *rig.control.scans.lock().unwrap() = vec![state(false, vec![item(1, "s:1", false, None)])];
    frames(&rig, &[1]);
    let model = Scripted::new(vec![calls(vec![bench(json!({ "query": "ligue1+", "fournisseur": "secours", "comparer_au_direct": true }))]), calls(vec![("reply", json!({ "message": "ok" }))])]);
    let mut plateau = direct("", "", "", "");
    plateau.kind = "plateau".into();
    readings(&model, vec![plateau, direct("Lens", "Paris FC", "1-0", "31:30")]);
    rig.use_model(std::sync::Arc::new(model.clone()));

    rig.hear("m-1", "Mamie", "retrouve le match");
    rig.drain();

    let out = result_of(&model, 0, 0);
    assert_eq!(out["reference"]["source"], "mémoire");
    assert_eq!(out["groupes"][0]["meme_que_direct"], "oui");
}

#[test]
fn a_vision_failure_on_one_channel_is_counted_and_the_others_are_still_read() {
    let rig = rig_with_backup();
    *rig.control.scans.lock().unwrap() = vec![state(false, vec![item(1, "s:1", false, None), item(2, "s:2", false, None)])];
    frames(&rig, &[1, 2]);
    let model = Scripted::new(vec![calls(vec![bench(json!({ "query": "ligue1+", "fournisseur": "secours" }))]), calls(vec![("reply", json!({ "message": "ok" }))])]);
    model.0.screens.lock().unwrap().push_back(Err(crate::model::ModelError::Transient("boom".into())));
    model.0.screens.lock().unwrap().push_back(Ok(direct("Lens", "Paris FC", "1-0", "34:12")));
    rig.use_model(std::sync::Arc::new(model.clone()));

    rig.hear("m-1", "Mamie", "scanne");
    rig.drain();

    let out = result_of(&model, 0, 0);
    assert_eq!((out["vision_erreurs"].clone(), out["groupes"].as_array().unwrap().len()), (json!(1), 2));
}
