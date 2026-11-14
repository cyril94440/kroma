use serde_json::json;

use super::support::{call, run};
use crate::model::WebAnswer;
use crate::testing::{programme, Rig, Scripted, START};

fn rig_with(programmes: Vec<crate::peers::Programme>) -> Rig {
    let rig = Rig::new();
    *rig.control.programmes.lock().unwrap() = programmes;
    rig
}

#[test]
fn search_guide_lists_live_first_drops_past_and_duplicates() {
    let rig = rig_with(vec![
        programme("tf1", "TF1", "Rugby France Australie", 120, 200),
        programme("m6", "M6", "Rugby France Australie", 120, 200),
        programme("arte", "Arte", "Rugby : le magazine", -30, 30),
        programme("tf1", "TF1", "Rugby hier", -300, -200),
    ]);
    let out = call(&rig, "search_guide", json!({ "query": "rugby" }), "Mamie");
    let titles: Vec<&str> = out["programmes"].as_array().unwrap().iter().map(|p| p["titre"].as_str().unwrap()).collect();
    assert_eq!(titles, vec!["Rugby : le magazine", "Rugby France Australie"]);
    assert_eq!(out["programmes"][0]["en_direct"], true);
    assert!(out["heure_actuelle"].as_str().unwrap().contains("novembre 2026"));
}

#[test]
fn search_guide_respects_its_limit_and_needs_a_query() {
    let rig = rig_with((0..5).map(|i| programme("tf1", "TF1", &format!("Foot {i}"), 10 + i, 90)).collect());
    assert_eq!(call(&rig, "search_guide", json!({ "query": "foot", "limit": 2 }), "Mamie")["programmes"].as_array().unwrap().len(), 2);
    assert_eq!(call(&rig, "search_guide", json!({}), "Mamie")["ok"], false);
}

#[test]
fn find_program_puts_the_only_live_match_to_the_vote() {
    let rig = rig_with(vec![programme("m6", "M6", "Lens - PSG", -20, 70)]);
    let out = call(&rig, "find_program", json!({ "query": "lens psg" }), "Mamie");
    assert_eq!(out["status"], "vote_open");
    assert_eq!(out["program"], "Lens - PSG");
    assert_eq!(rig.room.switches.lock().unwrap()[0].0, "m6");
}

#[test]
fn two_different_live_programmes_are_not_guessed() {
    let rig = rig_with(vec![programme("tf1", "TF1", "Rugby : France / Australie", -20, 70), programme("m6", "M6", "Football : France / Australie", -20, 70)]);
    let out = call(&rig, "find_program", json!({ "query": "france australie" }), "Mamie");
    assert_eq!(out["status"], "ambiguous");
    assert_eq!(out["suggestions"].as_array().unwrap().len(), 2);
    assert!(rig.room.switches.lock().unwrap().is_empty());
}

#[test]
fn the_title_equal_to_the_request_wins_over_a_longer_one() {
    let rig = rig_with(vec![programme("tf1", "TF1", "Tour de France : le mag", -20, 70), programme("m6", "M6", "Tour de France", -20, 70)]);
    let out = call(&rig, "find_program", json!({ "query": "tour de france" }), "Mamie");
    assert_eq!(out["status"], "vote_open");
    assert_eq!(rig.room.switches.lock().unwrap()[0].0, "m6");
}

#[test]
fn a_foreign_only_programme_is_not_put_unless_the_model_insists() {
    let rig = rig_with(vec![programme("uk1", "UK: BBC One", "Lens - PSG", -20, 70)]);
    let out = call(&rig, "find_program", json!({ "query": "lens psg" }), "Mamie");
    assert_eq!(out["status"], "seulement_etranger");
    assert!(out["consigne"].as_str().unwrap().contains("FRANÇAIS"));
    assert!(rig.room.switches.lock().unwrap().is_empty());
    let forced = call(&rig, "find_program", json!({ "query": "lens psg", "etrangere_ok": true }), "Mamie");
    assert_eq!(forced["status"], "vote_open");
}

#[test]
fn an_upcoming_programme_is_announced_without_changing_channel() {
    let rig = rig_with(vec![programme("m6", "M6", "Lens - PSG", 90, 180)]);
    let (out, ctx) = run(&rig, &Scripted::new(Vec::new()), "find_program", json!({ "query": "lens psg" }), "Mamie");
    assert_eq!(out["status"], "upcoming");
    assert!(rig.room.switches.lock().unwrap().is_empty());
    assert!(ctx.answered);
    let said = rig.room.texts();
    assert!(said[0].contains("Lens - PSG") && said[0].contains("M6") && said[0].starts_with("@Mamie"), "{said:?}");
    assert!(said[0].contains("23h50"), "{said:?}");
}

#[test]
fn partial_matches_are_proposed_and_never_put() {
    let rig = rig_with(vec![programme("m6", "M6", "France / Australie", -20, 70)]);
    let out = call(&rig, "find_program", json!({ "query": "tour de france" }), "Mamie");
    assert_eq!(out["status"], "partial_matches");
    assert_eq!(out["suggestions"][0]["programme"], "France / Australie");
    assert!(rig.room.switches.lock().unwrap().is_empty());
}

#[test]
fn nothing_found_returns_a_way_forward() {
    let rig = rig_with(Vec::new());
    let out = call(&rig, "find_program", json!({ "query": "pétanque" }), "Mamie");
    assert_eq!(out["status"], "not_found");
    assert!(out["suite_possible"].as_str().unwrap().contains("web_search"));
}

#[test]
fn a_vague_request_is_sharpened_by_a_web_search_then_found_in_the_guide() {
    let rig = rig_with(vec![programme("m6", "M6", "Argentine - Egypte", -10, 80)]);
    let model = Scripted::new(Vec::new());
    *model.0.search.lock().unwrap() = Some(Ok(WebAnswer { text: "Argentine, Egypte, Coupe du Monde U20".into(), sources: vec![] }));
    let (out, _) = run(&rig, &model, "find_program", json!({ "query": "le match de ce soir de l'albiceleste" }), "Mamie");
    assert_eq!(out["status"], "vote_open", "{out}");
    let (again, _) = run(&rig, &model, "find_program", json!({ "query": "le match de ce soir de l'albiceleste" }), "Papi");
    assert_eq!(again["status"], "vote_open");
    assert_eq!(model.0.searches.lock().unwrap().len(), 1);
    assert!(rig.deps.now() >= START);
}
