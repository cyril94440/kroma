use serde_json::json;

use super::loops::{calls, fin, play, result_of};
use crate::model::{ModelError, ToolCall, ToolDecl};
use crate::peers::ExternalTool;
use crate::testing::{Rig, Scripted, Step};

#[test]
fn a_prewritten_reply_is_skipped_when_a_refusable_tool_is_refused() {
    type Refusal = (&'static str, serde_json::Value, Box<dyn Fn(&Rig)>);
    let refusals: Vec<Refusal> = vec![
        ("record_poll_vote", json!({ "value": "Brésil" }), Box::new(|r: &Rig| *r.room.vote_error.lock().unwrap() = Some("noActivePoll".into()))),
        ("kick_from_party", json!({ "pseudo": "Papi" }), Box::new(|r: &Rig| *r.room.kick_reply.lock().unwrap() = Some(crate::peers::KickOutcome { outcome: "refused".into(), reason: Some("notInParty".into()), disruptions: 0 }))),
        ("start_poll", json!({ "question": "Qui ?", "type": "open" }), Box::new(|r: &Rig| *r.room.poll_error.lock().unwrap() = Some("pollAlreadyActive".into()))),
        ("kick_from_party", json!({ "pseudo": "Inconnu" }), Box::new(|_: &Rig| {})),
    ];
    for (tool, args, setup) in refusals {
        let rig = Rig::new();
        setup(&rig);
        let model = play(&rig, "Hermès ok va", vec![calls(vec![(tool_name(tool), args), ("reply", json!({ "message": "C'est noté !" }))]), calls(vec![("reply", json!({ "message": "Désolé, ça n'a pas marché." }))])]);
        assert_eq!(rig.room.texts(), vec!["Désolé, ça n'a pas marché.".to_string()], "{tool}");
        assert!(result_of(&model, 0, 1).as_str().unwrap().contains("non posté"), "{tool}");
    }
}

fn tool_name(name: &str) -> &'static str {
    match name {
        "record_poll_vote" => "record_poll_vote",
        "kick_from_party" => "kick_from_party",
        _ => "start_poll",
    }
}

#[test]
fn when_the_tool_succeeds_the_reply_goes_out_as_the_one_answer() {
    let rig = Rig::new();
    play(&rig, "Hermès je vote Brésil", vec![calls(vec![("record_poll_vote", json!({ "value": "Brésil" })), ("reply", json!({ "message": "C'est noté !" }))])]);
    assert_eq!(fin(&rig), "terminal");
    assert_eq!(rig.room.texts(), vec!["C'est noté !".to_string()]);
}

#[test]
fn a_peer_error_that_names_a_stream_url_is_masked_for_the_model_and_the_log() {
    let rig = Rig::new();
    rig.control.status.lock().unwrap().state = "running".into();
    *rig.control.down.lock().unwrap() = true;
    let model = play(&rig, "c'est quoi cette chaîne ?", vec![calls(vec![("get_status", json!({}))]), calls(vec![("reply", json!({ "message": "souci technique" }))])]);
    let texte = format!("{:?}{:?}", model.0.answers.lock().unwrap(), rig.deps.store.audit_rows(None, None, 200).unwrap());
    assert!(texte.contains("indisponible"));
    let rig = Rig::new();
    struct Leaky;
    impl crate::peers::Control for Leaky {
        fn status(&self) -> anyhow::Result<crate::peers::Status> {
            Err(anyhow::anyhow!("read failed on http://h:80/TESTUSER/TESTPASS42/1.ts"))
        }
        fn resolve(&self, _: &str, _: u32) -> anyhow::Result<Vec<crate::peers::Candidate>> {
            Ok(Vec::new())
        }
        fn epg(&self, _: Option<&str>, _: Option<i64>, _: Option<i64>, _: u32) -> anyhow::Result<Vec<crate::peers::Programme>> {
            Ok(Vec::new())
        }
        fn snapshot(&self) -> anyhow::Result<crate::peers::Snapshot> {
            anyhow::bail!("no")
        }
    }
    let deps = std::sync::Arc::new(crate::agent::Deps::new(rig.deps.host.clone(), rig.room.clone(), std::sync::Arc::new(Leaky), rig.contributors.clone(), rig.deps.store.clone(), rig.deps.clock.clone(), rig.deps.models.clone()));
    let agent = crate::agent::Agent::new(deps.clone());
    let model = Scripted::new(vec![calls(vec![("get_status", json!({}))]), calls(vec![("reply", json!({ "message": "souci" }))])]);
    rig.use_model(std::sync::Arc::new(model.clone()));
    let message = crate::store::Incoming { id: "x".into(), at: rig.now(), user_id: "u-mamie".into(), name: "Mamie".into(), text: "c'est quoi ?".into() };
    deps.store.claim(&message, crate::store::State::Queued, None, rig.now()).unwrap();
    for _ in 0..20 {
        agent.tick();
    }
    let shown = format!("{:?}{:?}", model.0.answers.lock().unwrap(), deps.store.audit_rows(None, None, 200).unwrap());
    assert!(!shown.contains("TESTUSER") && !shown.contains("TESTPASS42"), "{shown}");
    assert!(shown.contains("<url>"));
}

#[test]
fn malformed_arguments_and_unknown_tools_come_back_as_errors_the_model_can_fix() {
    let rig = Rig::new();
    let bad = ToolCall { id: "a".into(), name: "reply".into(), args: json!({}), malformed: Some("the arguments are not valid JSON".into()) };
    let model = play(&rig, "mets tf1", vec![Step::Raw(vec![bad, ToolCall { id: "b".into(), ..ToolCall::new("teleport", json!({})) }]), calls(vec![("reply", json!({ "message": "Je corrige." }))])]);
    assert!(result_of(&model, 0, 0)["erreur"].as_str().unwrap().contains("arguments illisibles"));
    assert!(result_of(&model, 0, 1)["erreur"].as_str().unwrap().contains("outil inconnu"));
    assert_eq!(rig.room.texts(), vec!["Je corrige.".to_string()]);
}

#[test]
fn a_contributed_tool_is_called_with_the_authors_context_and_its_result_returns_to_the_model() {
    let rig = Rig::new();
    *rig.contributors.tools.lock().unwrap() = vec![ExternalTool { module_id: "tv.kroma.sport".into(), decl: ToolDecl { name: "match_facts".into(), description: "Faits d'un match".into(), parameters: json!({ "type": "object", "properties": { "equipes": { "type": "string" } } }) } }];
    *rig.contributors.answer.lock().unwrap() = Some(Ok(json!({ "score": "1-0" })));
    let model = play(&rig, "quel score pour Lens ?", vec![calls(vec![("match_facts", json!({ "equipes": "Lens PSG" }))]), calls(vec![("reply", json!({ "message": "1-0" }))])]);
    assert_eq!(result_of(&model, 0, 0), json!({ "score": "1-0" }));
    let called = rig.contributors.calls.lock().unwrap().clone();
    assert_eq!(called[0], ("tv.kroma.sport".to_string(), "match_facts".to_string(), json!({ "equipes": "Lens PSG" }), "u-mamie".to_string(), "m-1".to_string()));
    assert!(model.0.opened.lock().unwrap()[0].2.contains(&"match_facts".to_string()));
    assert!(model.0.opened.lock().unwrap()[0].0.contains("D'autres outils t'ont été confiés"));
}

#[test]
fn a_failing_contributor_is_reported_to_the_model_not_hidden() {
    let rig = Rig::new();
    *rig.contributors.tools.lock().unwrap() = vec![ExternalTool { module_id: "tv.kroma.sport".into(), decl: ToolDecl { name: "match_facts".into(), description: "Faits".into(), parameters: json!({ "type": "object" }) } }];
    *rig.contributors.answer.lock().unwrap() = Some(Err("HTTP 500".into()));
    let model = play(&rig, "quel score ?", vec![calls(vec![("match_facts", json!({}))]), calls(vec![("reply", json!({ "message": "Je ne sais pas." }))])]);
    assert_eq!(result_of(&model, 0, 0)["ok"], false);
}

#[test]
fn an_outage_posts_one_notice_then_stays_quiet_for_ten_minutes() {
    let rig = Rig::new();
    play(&rig, "mets tf1", vec![Step::Fail(ModelError::Transient("down".into()))]);
    let notice = crate::strings::text("fr", "hermes.fallback.unavailable", &[]);
    assert_eq!(rig.room.texts(), vec![notice.clone()]);
    rig.advance(10_000);
    rig.use_model(std::sync::Arc::new(Scripted::new(vec![Step::Fail(ModelError::RateLimited)])));
    rig.hear("m-2", "Papi", "mets m6");
    rig.drain();
    assert_eq!(rig.room.texts(), vec![notice], "one notice per ten minutes, not one per message");
}

#[test]
fn a_user_over_the_per_minute_limit_is_skipped_with_its_reason_in_the_log() {
    let rig = Rig::with_config(&[("userTurnsPerMinute", json!(2))]);
    rig.use_model(std::sync::Arc::new(Scripted::new((0..4).map(|_| calls(vec![("ignore", json!({}))])).collect())));
    for i in 0..4 {
        rig.hear(&format!("m-{i}"), "Mamie", "mets tf1");
        rig.advance(1000);
    }
    rig.drain();
    let rows = rig.deps.store.inbox_recent(10).unwrap();
    assert_eq!(rows.iter().filter(|r| r.reason.as_deref() == Some("user_rate_limited")).count(), 2);
    assert_eq!(rig.turns().len(), 2);
}

#[test]
fn the_hourly_call_limit_stops_new_turns_and_says_so_once() {
    let rig = Rig::with_config(&[("hourlyCallLimit", json!(1))]);
    let model = Scripted::new(vec![calls(vec![("get_status", json!({}))]), calls(vec![("reply", json!({ "message": "ok" }))]), calls(vec![("reply", json!({ "message": "encore" }))])]);
    rig.use_model(std::sync::Arc::new(model.clone()));
    rig.hear("m-1", "Mamie", "mets tf1");
    rig.drain();
    rig.hear("m-2", "Papi", "mets m6");
    rig.drain();
    rig.hear("m-3", "Mamie", "mets arte");
    rig.drain();
    let texts = rig.room.texts();
    assert_eq!(texts.iter().filter(|t| **t == crate::strings::text("fr", "hermes.fallback.limit", &[])).count(), 1, "{texts:?}");
    let reasons: Vec<Option<String>> = rig.deps.store.inbox_recent(10).unwrap().into_iter().map(|r| r.reason).collect();
    assert!(reasons.contains(&Some("hourly_limit".to_string())));
}

#[test]
fn the_activity_line_names_the_tool_then_clears() {
    let rig = Rig::new();
    play(&rig, "c'est quoi ?", vec![calls(vec![("get_status", json!({}))]), calls(vec![("search_guide", json!({ "query": "journal" }))]), calls(vec![("reply", json!({ "message": "Le journal." }))])]);
    let log = rig.room.activity_log.lock().unwrap().clone();
    assert_eq!(log, vec![Some("réfléchit".to_string()), Some("vérifie l'état du flux".to_string()), Some("consulte le guide TV".to_string()), Some("réfléchit".to_string()), None]);
}

#[test]
fn stopping_the_module_mid_turn_marks_the_message_interrupted_and_it_is_not_redone() {
    let rig = Rig::new();
    rig.use_model(std::sync::Arc::new(Scripted::new((0..3).map(|_| calls(vec![("get_status", json!({}))])).collect())));
    rig.hear("m-1", "Mamie", "regarde");
    rig.agent.tick();
    rig.agent.tick();
    assert!(rig.agent.is_busy());
    rig.agent.stop();
    assert!(!rig.agent.is_busy());
    let row = rig.deps.store.inbox_recent(5).unwrap().remove(0);
    assert_eq!(row.state, "interrupted");
    rig.drain();
    assert_eq!(rig.deps.store.interrupt_active(1).unwrap(), 0);
}

#[test]
fn an_unconfigured_model_skips_the_message_and_logs_it_once() {
    let rig = Rig::new();
    rig.hear("m-1", "Mamie", "mets tf1");
    rig.hear("m-2", "Papi", "mets m6");
    rig.drain();
    let rows = rig.deps.store.inbox_recent(5).unwrap();
    assert!(rows.iter().all(|r| r.reason.as_deref() == Some("not_configured")));
    let logged = rig.deps.store.audit_rows(Some("audit"), None, 20).unwrap().into_iter().filter(|r| r.event == "not_configured").count();
    assert_eq!(logged, 1);
    assert!(rig.room.texts().is_empty());
}
