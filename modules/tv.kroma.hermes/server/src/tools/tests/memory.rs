use serde_json::json;

use super::support::call;
use crate::testing::{Rig, START};

#[test]
fn a_lesson_is_dated_stored_and_shown_in_the_next_prompts_rules() {
    let rig = Rig::new();
    let out = call(&rig, "learn_lesson", json!({ "lesson": "Toujours dire la chaîne et l'heure du prochain match." }), "Mamie");
    assert_eq!(out["ok"], true);
    assert_eq!(out["lecons_total"], 1);
    let rendered = rig.deps.rules.render();
    assert!(rendered.contains("## Leçons apprises (par Hermès)"));
    assert!(rendered.contains("[06/11/2026] Toujours dire la chaîne"), "{rendered}");
}

#[test]
fn a_lesson_outside_its_bounds_is_refused() {
    let rig = Rig::new();
    assert_eq!(call(&rig, "learn_lesson", json!({ "lesson": "court" }), "Mamie")["ok"], false);
    assert_eq!(call(&rig, "learn_lesson", json!({ "lesson": "x".repeat(301) }), "Mamie")["ok"], false);
}

#[test]
fn lessons_are_a_fifo_of_thirty() {
    let rig = Rig::new();
    for i in 0..35 {
        call(&rig, "learn_lesson", json!({ "lesson": format!("Leçon numéro {i:02} pour la maison") }), "Mamie");
    }
    let lessons = rig.deps.store.lessons().unwrap();
    assert_eq!(lessons.len(), 30);
    assert!(lessons[0].text.contains("05") && lessons[29].text.contains("34"));
}

#[test]
fn the_shipped_lessons_are_seeded_once() {
    let rig = Rig::new();
    rig.deps.rules.seed_lessons(START).unwrap();
    rig.deps.rules.seed_lessons(START).unwrap();
    let lessons = rig.deps.store.lessons().unwrap();
    assert_eq!(lessons.len(), 17);
    assert!(lessons.iter().any(|l| l.text.contains("BEIN SPORTS")));
}

#[test]
fn a_suggestion_is_stored_and_the_same_one_within_half_an_hour_is_not_noted_twice() {
    let rig = Rig::new();
    let args = json!({ "kind": "bug", "titre": "Plugin Jellyfin absent", "detail": "Papi demande un plugin Jellyfin depuis hier." });
    assert_eq!(call(&rig, "save_suggestion", args.clone(), "Papi")["total"], 1);
    assert_eq!(call(&rig, "save_suggestion", args.clone(), "Papi")["deja_notee"], true);
    rig.advance(31 * 60_000);
    assert_eq!(call(&rig, "save_suggestion", args, "Papi")["total"], 2);
    assert_eq!(rig.deps.store.suggestions().unwrap()[0].kind, "bug");
}

#[test]
fn a_suggestion_needs_a_real_title_and_detail_and_unknown_kinds_become_observations() {
    let rig = Rig::new();
    assert_eq!(call(&rig, "save_suggestion", json!({ "kind": "bug", "titre": "abc", "detail": "trop court" }), "Papi")["ok"], false);
    call(&rig, "save_suggestion", json!({ "kind": "wish", "titre": "Titre correct", "detail": "Un détail assez long." }), "Papi");
    assert_eq!(rig.deps.store.suggestions().unwrap()[0].kind, "observation");
}

#[test]
fn notes_are_about_the_author_only_and_can_be_forgotten() {
    let rig = Rig::new();
    assert_eq!(call(&rig, "note_person", json!({ "note": "Supporter du RC Lens depuis toujours" }), "Mamie")["ok"], true);
    call(&rig, "note_person", json!({ "note": "Préfère les films en version française" }), "Papi");
    assert_eq!(rig.deps.store.notes("u-mamie").unwrap(), vec!["Supporter du RC Lens depuis toujours".to_string()]);
    assert_eq!(call(&rig, "note_person", json!({ "note": "court" }), "Mamie")["ok"], false);
    assert_eq!(call(&rig, "note_person", json!({ "forget": true }), "Mamie")["oubliees"], 1);
    assert!(rig.deps.store.notes("u-mamie").unwrap().is_empty());
    assert_eq!(rig.deps.store.notes("u-papi").unwrap().len(), 1);
}

#[test]
fn a_lesson_already_known_is_not_stored_twice_so_a_repeat_cannot_push_the_others_out() {
    let rig = Rig::new();
    call(&rig, "learn_lesson", json!({ "lesson": "Toujours dire la chaîne et l'heure du prochain match." }), "Mamie");

    let again = call(&rig, "learn_lesson", json!({ "lesson": "toujours dire la chaîne et l'heure du prochain match." }), "Papi");

    assert_eq!((again["ok"].clone(), again["deja_retenue"].clone()), (json!(true), json!(true)));
    assert_eq!(rig.deps.store.lessons().unwrap().len(), 1);
}
