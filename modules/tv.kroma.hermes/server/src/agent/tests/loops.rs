use serde_json::{json, Value};

use crate::model::ModelError;
use crate::testing::{programme, Rig, Scripted, Step};

pub fn play(rig: &Rig, text: &str, steps: Vec<Step>) -> Scripted {
    let model = Scripted::new(steps);
    rig.use_model(std::sync::Arc::new(model.clone()));
    rig.hear("m-1", "Mamie", text);
    rig.drain();
    model
}

pub fn calls(list: Vec<(&'static str, Value)>) -> Step {
    Step::Calls(list)
}

pub fn fin(rig: &Rig) -> String {
    rig.turns().last().unwrap()["fin"].as_str().unwrap().to_string()
}

pub fn result_of(model: &Scripted, step: usize, call: usize) -> Value {
    model.0.answers.lock().unwrap()[step][call].1.clone()
}

#[test]
fn a_refused_channel_is_investigated_then_the_right_one_is_asked() {
    let rig = Rig::new();
    let model = play(
        &rig,
        "mets Ligue1+ 4",
        vec![
            calls(vec![("change_channel", json!({ "query": "ligue1+ 4" }))]),
            calls(vec![("list_channels", json!({ "query": "tf1" }))]),
            calls(vec![("change_channel", json!({ "ref": "tf1" }))]),
        ],
    );
    assert_eq!(fin(&rig), "terminal");
    assert_eq!(result_of(&model, 0, 0)["status"], "not_found");
    assert_eq!(rig.room.switches.lock().unwrap().len(), 1);
    assert!(rig.room.texts().is_empty());
}

#[test]
fn a_failure_then_silence_posts_the_fallback_once() {
    let rig = Rig::new();
    play(&rig, "mets Artee", vec![calls(vec![("change_channel", json!({ "query": "artee" }))]), Step::Nothing]);
    assert_eq!(rig.room.texts(), vec![crate::strings::text("fr", "hermes.fallback.timeout", &[])]);
    assert_eq!(fin(&rig), "sans_outil");
}

#[test]
fn a_reply_with_an_investigation_is_a_waiting_message_then_the_answer() {
    let rig = Rig::new();
    let model = play(&rig, "c'est quoi ce film ?", vec![calls(vec![("get_status", json!({})), ("reply", json!({ "message": "Je regarde…" }))]), calls(vec![("reply", json!({ "message": "Un journal." }))])]);
    assert_eq!(rig.room.texts(), vec!["Je regarde…".to_string(), "Un journal.".to_string()]);
    assert_eq!(fin(&rig), "terminal");
    assert_eq!(model.0.answers.lock().unwrap()[0].len(), 2);
}

#[test]
fn the_waiting_message_goes_first_and_a_duplicate_reply_is_posted_once() {
    let rig = Rig::new();
    let model = play(
        &rig,
        "scanne",
        vec![
            calls(vec![("get_status", json!({})), ("reply", json!({ "message": "Je regarde…" })), ("reply", json!({ "message": "Je regarde…" }))]),
            calls(vec![("reply", json!({ "message": "Rien de spécial." }))]),
        ],
    );
    assert_eq!(rig.room.texts(), vec!["Je regarde…".to_string(), "Rien de spécial.".to_string()]);
    assert_eq!(model.0.answers.lock().unwrap()[0].len(), 3, "each model call gets its result");
    assert_eq!(rig.room.said.lock().unwrap()[0].reply_to.as_deref(), Some("m-1"));
}

#[test]
fn an_action_and_its_reply_keep_their_order() {
    let rig = Rig::new();
    play(&rig, "mets arte", vec![calls(vec![("change_channel", json!({ "query": "arte" })), ("reply", json!({ "message": "Vote lancé" }))])]);
    assert_eq!(rig.room.switches.lock().unwrap().len(), 1);
    assert_eq!(rig.room.texts(), vec!["Vote lancé".to_string()]);
    assert_eq!(fin(&rig), "terminal");
}

#[test]
fn only_one_broadcast_action_per_message() {
    let rig = Rig::new();
    let model = play(
        &rig,
        "mets m6 ou arte",
        vec![calls(vec![("change_channel", json!({ "query": "m6" })), ("change_channel", json!({ "query": "arte" }))]), calls(vec![("reply", json!({ "message": "ok" }))])],
    );
    assert_eq!(rig.room.switches.lock().unwrap().len(), 1);
    assert_eq!(result_of(&model, 0, 1)["status"], "refus");
}

#[test]
fn ten_steps_then_only_the_final_tools_are_allowed() {
    let rig = Rig::new();
    let model = play(&rig, "regarde", (0..12).map(|_| calls(vec![("get_status", json!({}))])).collect());
    let allowed = model.0.allowed.lock().unwrap().clone();
    assert_eq!(allowed.len(), 10);
    assert!(allowed[..9].iter().all(|a| *a == crate::model::Allowed::Any));
    assert!(matches!(&allowed[9], crate::model::Allowed::Only(names) if names.contains(&"reply".to_string()) && !names.contains(&"get_status".to_string())));
    assert_eq!(fin(&rig), "sans_outil");
    assert_eq!(rig.room.texts(), vec![crate::strings::text("fr", "hermes.fallback.timeout", &[])]);
    assert_eq!(model.remaining(), 2);
}

#[test]
fn past_the_time_budget_only_an_answer_is_allowed() {
    let rig = Rig::new();
    let model = Scripted::new((0..5).map(|_| calls(vec![("get_status", json!({}))])).collect());
    *model.0.per_step.lock().unwrap() = Some((rig.clock.clone(), 70_000));
    rig.use_model(std::sync::Arc::new(model.clone()));
    rig.hear("m-1", "Mamie", "regarde");
    rig.drain();
    assert!(matches!(model.0.allowed.lock().unwrap()[2], crate::model::Allowed::Only(_)));
    assert_eq!(rig.room.texts(), vec![crate::strings::text("fr", "hermes.fallback.timeout", &[])]);
}

#[test]
fn a_very_long_step_still_gets_one_last_answer() {
    let rig = Rig::new();
    let model = Scripted::new(vec![calls(vec![("get_status", json!({}))]), calls(vec![("reply", json!({ "message": "voici ce que j'ai vu" }))])]);
    *model.0.per_step.lock().unwrap() = Some((rig.clock.clone(), 200_000));
    rig.use_model(std::sync::Arc::new(model.clone()));
    rig.hear("m-1", "Mamie", "regarde");
    rig.drain();
    assert_eq!(rig.room.texts(), vec!["voici ce que j'ai vu".to_string()]);
    assert_eq!(fin(&rig), "terminal");
    let allowed = model.0.allowed.lock().unwrap().clone();
    assert_eq!((allowed[0] == crate::model::Allowed::Any, matches!(allowed[1], crate::model::Allowed::Only(_))), (true, true));
}

#[test]
fn a_last_answer_that_says_nothing_gets_the_fallback() {
    let rig = Rig::new();
    let model = Scripted::new(vec![calls(vec![("get_status", json!({}))]), Step::Nothing]);
    *model.0.per_step.lock().unwrap() = Some((rig.clock.clone(), 200_000));
    rig.use_model(std::sync::Arc::new(model));
    rig.hear("m-1", "Mamie", "regarde");
    rig.drain();
    assert_eq!(rig.room.texts(), vec![crate::strings::text("fr", "hermes.fallback.timeout", &[])]);
    assert_eq!(fin(&rig), "sans_outil");
}

#[test]
fn a_turn_whose_deadline_passed_before_the_first_call_ends_silently() {
    let rig = Rig::new();
    let model = Scripted::new(vec![calls(vec![("get_status", json!({}))])]);
    rig.use_model(std::sync::Arc::new(model.clone()));
    rig.hear("m-1", "Mamie", "regarde");
    rig.agent.tick();
    rig.advance(400_000);
    rig.drain();
    assert_eq!(fin(&rig), "budget");
    assert!(rig.room.texts().is_empty());
    assert_eq!(model.remaining(), 1);
}

#[test]
fn ignore_alone_posts_nothing_and_after_an_investigation_stays_mute() {
    let rig = Rig::new();
    play(&rig, "le flux est noir chez toi ?", vec![calls(vec![("get_status", json!({}))]), calls(vec![("ignore", json!({ "reason": "entre eux" }))])]);
    assert!(rig.room.texts().is_empty());
    assert_eq!(fin(&rig), "terminal");
    let decisions = rig.deps.store.audit_rows(Some("decisions"), None, 5).unwrap();
    assert_eq!(decisions[0].event, "ignore");
}

#[test]
fn a_waiting_message_then_ignore_ends_with_a_neutral_conclusion() {
    let rig = Rig::new();
    play(&rig, "regarde ça", vec![calls(vec![("get_status", json!({})), ("reply", json!({ "message": "Je regarde…" }))]), calls(vec![("ignore", json!({}))])]);
    assert_eq!(rig.room.texts(), vec!["Je regarde…".to_string(), crate::strings::text("fr", "hermes.fallback.nothing", &[])]);
}

#[test]
fn a_model_error_before_any_tool_stays_silent_unless_it_is_an_outage() {
    let rig = Rig::new();
    play(&rig, "mets tf1", vec![Step::Fail(ModelError::Rejected("bad key".into()))]);
    assert!(rig.room.texts().is_empty());
    assert_eq!(fin(&rig), "erreur");
    let rows = rig.deps.store.audit_rows(Some("audit"), None, 10).unwrap();
    assert!(rows.iter().any(|r| r.event == "agent_error"));
}

#[test]
fn a_reply_with_an_effect_tool_stays_the_answer_after_it() {
    let rig = Rig::new();
    let model = play(&rig, "Hermès, je ne t'ai pas demandé ça", vec![calls(vec![("learn_lesson", json!({ "lesson": "Ne pas zapper sur une question" })), ("reply", json!({ "message": "Désolé, je retiens." }))])]);
    assert_eq!(fin(&rig), "terminal");
    assert_eq!(rig.room.texts(), vec!["Désolé, je retiens.".to_string()]);
    assert_eq!(model.0.allowed.lock().unwrap().len(), 1);
    assert_eq!(rig.deps.store.lessons().unwrap().len(), 1);
}

#[test]
fn a_reply_written_before_a_failure_is_not_posted() {
    let rig = Rig::new();
    let model = play(
        &rig,
        "mets artee",
        vec![calls(vec![("change_channel", json!({ "query": "artee" })), ("reply", json!({ "message": "Je mets Arte !" }))]), calls(vec![("reply", json!({ "message": "Je ne trouve pas « artee »." }))])],
    );
    assert_eq!(rig.room.texts(), vec!["Je ne trouve pas « artee »."]);
    assert!(result_of(&model, 0, 1).as_str().unwrap().contains("non posté"));
}

#[test]
fn a_reply_listed_before_the_action_is_held_back_the_same_way() {
    let rig = Rig::new();
    let model = play(&rig, "mets artee", vec![calls(vec![("reply", json!({ "message": "Je mets Artee !" })), ("change_channel", json!({ "query": "artee" }))]), calls(vec![("ignore", json!({}))])]);
    assert!(rig.room.texts().iter().all(|t| t != "Je mets Artee !"));
    assert!(result_of(&model, 0, 0).as_str().unwrap().contains("non posté"));
}

#[test]
fn the_waiting_message_comes_first_even_next_to_an_effect_tool() {
    let rig = Rig::new();
    play(
        &rig,
        "regarde",
        vec![
            calls(vec![("save_suggestion", json!({ "kind": "bug", "titre": "Un titre", "detail": "Un détail assez long." })), ("get_status", json!({})), ("reply", json!({ "message": "Je regarde…" }))]),
            calls(vec![("reply", json!({ "message": "Fini." }))]),
        ],
    );
    assert_eq!(rig.room.texts(), vec!["Je regarde…".to_string(), "Fini.".to_string()]);
}

#[test]
fn a_partial_match_forbids_any_broadcast_action_later_in_the_turn() {
    let rig = Rig::new();
    *rig.control.programmes.lock().unwrap() = vec![programme("m6", "M6", "France / Australie", -20, 70)];
    let model = play(
        &rig,
        "remets le tour de france",
        vec![
            calls(vec![("find_program", json!({ "query": "tour de france" }))]),
            calls(vec![("change_channel", json!({ "query": "m6" }))]),
            calls(vec![("find_program", json!({ "query": "France / Australie" }))]),
            calls(vec![("reply", json!({ "message": "Seulement du rugby : je le mets ?" }))]),
        ],
    );
    assert!(rig.room.switches.lock().unwrap().is_empty());
    assert!(result_of(&model, 1, 0)["raison"].as_str().unwrap().contains("PARTIELLE"));
    assert!(result_of(&model, 2, 0)["raison"].as_str().unwrap().contains("PARTIELLE"));
}

