use serde_json::json;

use crate::testing::{Rig, Scripted, Step};

#[test]
fn every_stage_of_a_turn_leaves_a_record_with_its_decisions_and_refusals() {
    let rig = Rig::new();
    let model = Scripted::new(vec![
        Step::Calls(vec![("change_channel", json!({ "query": "artee" }))]),
        Step::Calls(vec![("change_channel", json!({ "query": "arte" })), ("reply", json!({ "message": "Je lance le vote." }))]),
    ]);
    rig.use_model(std::sync::Arc::new(model));
    rig.hear("m-1", "Mamie", "mets artee");
    rig.drain();
    let all = rig.deps.store.audit_rows(None, None, 200).unwrap();
    let find = |genre: &str, event: &str| all.iter().find(|r| r.genre == genre && r.event == event).unwrap_or_else(|| panic!("{genre}/{event}"));
    assert_eq!(find("chat", "gate").data["verdict"], "Engage");
    assert_eq!(find("chat", "turn_started").data["user"], "u-mamie");
    assert!(find("chat", "turn_started").data["tools"].as_array().unwrap().iter().any(|t| t == "change_channel"));
    assert_eq!(find("decisions", "change_channel_refused").data["why"], "not found");
    assert_eq!(find("decisions", "request_switch").data["name"], "Arte");
    assert_eq!(find("llm", "call").data["ok"], true);
    let turn = find("audit", "agent_turn");
    assert_eq!(turn.data["fin"], "terminal");
    assert_eq!(turn.data["tools"], json!(["change_channel", "change_channel", "reply"]));
}

#[test]
fn raw_sources_and_measures_are_kept_in_their_own_genres() {
    let rig = Rig::new();
    let model = Scripted::new(vec![Step::Calls(vec![("list_channels", json!({ "query": "m6" })), ("run_ops", json!({ "action": "diagnose" }))]), Step::Calls(vec![("reply", json!({ "message": "ok" }))])]);
    rig.use_model(std::sync::Arc::new(model));
    rig.hear("m-1", "Mamie", "mets m6, ça rame");
    rig.drain();
    let genres = rig.deps.store.audit_genres().unwrap();
    for genre in ["audit", "chat", "llm", "sources"] {
        assert!(genres.iter().any(|g| g == genre), "{genre} in {genres:?}");
    }
    let sources: Vec<String> = rig.deps.store.audit_rows(Some("sources"), None, 10).unwrap().into_iter().map(|r| r.event).collect();
    assert!(sources.contains(&"list_channels".to_string()) && sources.contains(&"run_ops".to_string()));
}

#[test]
fn the_audit_log_never_holds_a_configured_key_nor_a_url() {
    let key = "AIza-secret-key-9999";
    let rig = Rig::with_config(&[("geminiApiKey", json!(key)), ("geminiModel", json!("m")), ("openaiApiKey", json!("sk-another-secret-1234"))]);
    let model = Scripted::new(vec![
        Step::Calls(vec![("learn_lesson", json!({ "lesson": format!("La clé {key} et le lien http://h:80/u/p/1.ts ne doivent jamais être stockés") })), ("web_search", json!({ "query": "météo" }))]),
        Step::Calls(vec![("reply", json!({ "message": format!("voici {key} http://x.example/a?token=zzz") }))]),
    ]);
    *model.0.search.lock().unwrap() = Some(Ok(crate::model::WebAnswer { text: format!("réponse avec {key}"), sources: vec![] }));
    rig.use_model(std::sync::Arc::new(model));
    rig.hear("m-1", "Mamie", &format!("mets tf1 {key} http://leak.example/x"));
    rig.drain();
    let stored = format!("{:?}", rig.deps.store.audit_rows(None, None, 500).unwrap());
    for secret in [key, "sk-another-secret-1234", "http://h:80", "leak.example", "token=zzz"] {
        assert!(!stored.contains(secret), "{secret} leaked into the audit log");
    }
    assert!(stored.contains("<url>") && stored.contains("***"));
    let lessons = format!("{:?}", rig.deps.store.lessons().unwrap());
    assert!(lessons.contains("La clé") && !lessons.contains(key) && !lessons.contains("http://h:80"), "{lessons}");
}
