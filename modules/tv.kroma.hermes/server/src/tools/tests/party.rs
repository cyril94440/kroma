use serde_json::json;

use super::support::call;
use crate::peers::KickOutcome;
use crate::testing::Rig;

#[test]
fn start_poll_forwards_the_question_and_clamps_the_duration() {
    let rig = Rig::new();
    let out = call(&rig, "start_poll", json!({ "question": "Qui gagne ?", "type": "choice", "options": ["Lens", "PSG", "Nul"], "duration_s": 5000 }), "Mamie");
    assert_eq!(out["ok"], true);
    let polls = rig.room.polls.lock().unwrap().clone();
    assert_eq!(polls[0], ("u-mamie".to_string(), "Qui gagne ?".to_string(), "choice".to_string(), vec!["Lens".to_string(), "PSG".to_string(), "Nul".to_string()], Some(900)));
}

#[test]
fn a_refused_poll_is_returned_with_the_rooms_reason() {
    let rig = Rig::new();
    *rig.room.poll_error.lock().unwrap() = Some("pollAlreadyActive".into());
    let out = call(&rig, "start_poll", json!({ "question": "Qui ?", "type": "open" }), "Mamie");
    assert_eq!((out["ok"].clone(), out["raison"].clone()), (json!(false), json!("sondage_deja_actif")));
}

#[test]
fn close_poll_returns_the_results_or_the_refusal() {
    let rig = Rig::new();
    assert_eq!(call(&rig, "close_poll", json!({}), "Mamie")["resultats"]["total"], 2);
    *rig.room.poll_error.lock().unwrap() = Some("noActivePoll".into());
    assert_eq!(call(&rig, "close_poll", json!({}), "Mamie")["ok"], false);
}

#[test]
fn a_poll_vote_is_always_the_authors() {
    let rig = Rig::new();
    let out = call(&rig, "record_poll_vote", json!({ "value": "2-1", "userId": "u-papi", "pseudo": "Papi" }), "Mamie");
    assert_eq!(out["ok"], true);
    assert_eq!(rig.room.votes.lock().unwrap().clone(), vec![("u-mamie".to_string(), "2-1".to_string())]);
    *rig.room.vote_error.lock().unwrap() = Some("noActivePoll".into());
    assert_eq!(call(&rig, "record_poll_vote", json!({ "value": "x" }), "Mamie")["ok"], false);
}

#[test]
fn kick_resolves_the_name_among_present_viewers_and_asks_the_room() {
    let rig = Rig::new();
    let out = call(&rig, "kick_from_party", json!({ "pseudo": "papi", "reason": "il fait ramer" }), "Mamie");
    assert_eq!(out["ok"], true);
    assert_eq!(out["perturbations_recentes"], 2);
    assert_eq!(rig.room.kicks.lock().unwrap().clone(), vec![("u-papi".to_string(), "u-mamie".to_string(), "il fait ramer".to_string())]);
}

#[test]
fn kick_refuses_unknown_and_ambiguous_names_without_calling_the_room() {
    let rig = Rig::new();
    let unknown = call(&rig, "kick_from_party", json!({ "pseudo": "Tonton" }), "Mamie");
    assert_eq!(unknown["ok"], false);
    assert!(unknown["erreur"].as_str().unwrap().contains("Mamie, Papi"));
    rig.control.status.lock().unwrap().viewers.push(crate::peers::Viewer { user_id: "u-pa2".into(), name: "Papa".into() });
    assert!(call(&rig, "kick_from_party", json!({ "pseudo": "pap" }), "Mamie")["erreur"].as_str().unwrap().contains("plusieurs"));
    assert!(rig.room.kicks.lock().unwrap().is_empty());
}

#[test]
fn the_rooms_own_refusal_is_reported_as_is() {
    let rig = Rig::new();
    *rig.room.kick_reply.lock().unwrap() = Some(KickOutcome { outcome: "refused".into(), reason: Some("notInParty".into()), disruptions: 0 });
    let out = call(&rig, "kick_from_party", json!({ "pseudo": "Papi" }), "Mamie");
    assert_eq!(out["ok"], false);
    assert_eq!(out["raison"], "pas_dans_la_watch_party");
}

#[test]
fn the_polls_and_the_kick_name_the_author_from_the_message_never_from_the_arguments() {
    let rig = Rig::new();
    call(&rig, "start_poll", json!({ "question": "Qui ?", "type": "open", "requestedBy": "u-cyril", "pseudo": "Cyril" }), "Mamie");
    call(&rig, "close_poll", json!({ "requestedBy": "u-cyril" }), "Mamie");

    assert_eq!(rig.room.polls.lock().unwrap()[0].0, "u-mamie");
    assert_eq!(*rig.room.closes.lock().unwrap(), vec!["u-mamie".to_string()]);
}

#[test]
fn the_rooms_refusals_reach_the_model_as_a_reason_and_a_message_to_relay() {
    let rig = Rig::new();
    let cases = [
        ("notPresent", "demandeur_absent"),
        ("pollAlreadyActive", "sondage_deja_actif"),
        ("noActivePoll", "aucun_sondage_actif"),
    ];

    for (code, reason) in cases {
        *rig.room.poll_error.lock().unwrap() = Some(code.into());
        for tool in ["start_poll", "close_poll"] {
            let out = call(&rig, tool, json!({ "question": "Qui ?", "type": "open" }), "Mamie");
            assert_eq!((out["ok"].clone(), out["status"].clone(), out["raison"].clone()), (json!(false), json!("refus"), json!(reason)), "{tool} {code}");
            assert!(out["message"].as_str().is_some_and(|m| !m.is_empty()));
        }
    }
}

#[test]
fn a_kick_refusal_of_the_room_is_mapped_to_a_reason_the_model_relays() {
    let rig = Rig::new();
    let cases = [("notPresent", "demandeur_absent"), ("notInParty", "pas_dans_la_watch_party"), ("disconnected", "deconnecte_pour_l_instant"), ("recentlyKicked", "deja_expulse_recemment")];

    for (code, reason) in cases {
        *rig.room.kick_reply.lock().unwrap() = Some(KickOutcome { outcome: "refused".into(), reason: Some(code.into()), disruptions: 0 });
        let out = call(&rig, "kick_from_party", json!({ "pseudo": "Papi" }), "Mamie");
        assert_eq!((out["ok"].clone(), out["raison"].clone()), (json!(false), json!(reason)), "{code}");
    }
}
