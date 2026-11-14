use std::time::Duration;

use serde_json::{json, Value};

use crate::model::tests::fake::{FakeModel, Reply, Req};
use crate::testing::{programme, Rig};

#[derive(Clone, Copy, Debug)]
enum Provider {
    Gemini,
    OpenAi,
}

const KEY: &str = "AIza-e2e-key-5555";

fn rig(provider: Provider, base: &str, extra: &[(&str, Value)]) -> Rig {
    let mut config: Vec<(&str, Value)> = match provider {
        Provider::Gemini => vec![("provider", json!("gemini")), ("geminiApiKey", json!(KEY)), ("geminiModel", json!("gemini-test")), ("geminiBaseUrl", json!(base))],
        Provider::OpenAi => vec![("provider", json!("openai")), ("openaiApiKey", json!(KEY)), ("openaiModel", json!("local")), ("openaiBaseUrl", json!(base))],
    };
    config.push(("modelTimeoutSeconds", json!(1)));
    config.extend(extra.iter().map(|(k, v)| (*k, v.clone())));
    Rig::with_config(&config)
}

fn step(provider: Provider, calls: &[(&str, Value)]) -> Reply {
    match provider {
        Provider::Gemini => Reply::Json(200, json!({ "candidates": [{ "content": { "role": "model", "parts": calls.iter().map(|(n, a)| json!({ "functionCall": { "name": n, "args": a } })).collect::<Vec<_>>() } }] })),
        Provider::OpenAi => Reply::Json(200, json!({ "choices": [{ "message": { "role": "assistant", "content": null, "tool_calls": calls.iter().enumerate().map(|(i, (n, a))| json!({ "id": format!("call_{i}"), "type": "function", "function": { "name": n, "arguments": a.to_string() } })).collect::<Vec<_>>() } }] })),
    }
}

fn results(provider: Provider, req: &Req) -> Vec<Value> {
    match provider {
        Provider::Gemini => req.body["contents"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["parts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["functionResponse"]["response"]["result"].clone())
            .collect(),
        Provider::OpenAi => req.body["messages"]
            .as_array()
            .unwrap()
            .iter()
            .rev()
            .take_while(|m| m["role"] == "tool")
            .map(|m| serde_json::from_str(m["content"].as_str().unwrap()).unwrap())
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect(),
    }
}

fn both(test: impl Fn(Provider)) {
    test(Provider::Gemini);
    test(Provider::OpenAi);
}

#[test]
fn a_single_tool_call_becomes_a_chat_reply() {
    both(|p| {
        let server = FakeModel::scripted(vec![step(p, &[("reply", json!({ "message": "Bonsoir Mamie !" }))])]);
        let rig = rig(p, &server.base, &[]);
        rig.hear("m-1", "Mamie", "Hermès bonsoir");
        rig.drain();
        assert_eq!(rig.room.texts(), vec!["Bonsoir Mamie !".to_string()], "{p:?}");
        assert_eq!(server.count(), 1);
        assert_eq!(rig.turns()[0]["fin"], "terminal");
    });
}

#[test]
fn a_multi_step_turn_returns_each_result_to_the_model() {
    both(|p| {
        let server = FakeModel::scripted(vec![step(p, &[("get_status", json!({}))]), step(p, &[("reply", json!({ "message": "TF1, le journal." }))])]);
        let rig = rig(p, &server.base, &[]);
        rig.hear("m-1", "Mamie", "c'est quoi cette chaîne ?");
        rig.drain();
        assert_eq!(rig.room.texts(), vec!["TF1, le journal.".to_string()]);
        let back = results(p, &server.requests()[1]);
        assert_eq!(back[0]["chaine"]["nom"], "TF1", "{p:?}");
        assert_eq!(back[0]["en_ce_moment"]["titre"], "Journal");
    });
}

#[test]
fn a_tool_failure_is_returned_with_its_reason_and_the_model_recovers() {
    both(|p| {
        let server = FakeModel::scripted(vec![
            step(p, &[("change_channel", json!({ "query": "chaine fantome" }))]),
            step(p, &[("list_channels", json!({ "query": "arte" }))]),
            step(p, &[("change_channel", json!({ "ref": "arte" }))]),
        ]);
        let rig = rig(p, &server.base, &[]);
        rig.hear("m-1", "Mamie", "mets la chaîne fantôme");
        rig.drain();
        assert_eq!(results(p, &server.requests()[1])[0]["status"], "not_found", "{p:?}");
        assert_eq!(results(p, &server.requests()[2])[0]["chaines"][0]["ref"], "arte");
        assert_eq!(rig.room.switches.lock().unwrap().clone(), vec![("arte".to_string(), "u-mamie".to_string(), String::new())]);
        assert_eq!(rig.turns()[0]["fin"], "terminal");
    });
}

#[test]
fn a_program_request_goes_through_the_guide_and_the_rooms_vote() {
    both(|p| {
        let server = FakeModel::scripted(vec![step(p, &[("find_program", json!({ "query": "lens psg" }))])]);
        let rig = rig(p, &server.base, &[]);
        *rig.control.programmes.lock().unwrap() = vec![programme("m6", "M6", "Lens - PSG", -20, 70)];
        rig.hear("m-1", "Mamie", "mets le match Lens PSG");
        rig.drain();
        assert_eq!(rig.room.switches.lock().unwrap()[0], ("m6".to_string(), "u-mamie".to_string(), "Lens - PSG".to_string()));
        assert_eq!(server.count(), 1, "{p:?}: the vote announces itself, the turn is over");
    });
}

#[test]
fn a_model_that_never_stops_investigating_is_cut_at_the_step_budget() {
    both(|p| {
        let server = FakeModel::scripted(vec![step(p, &[("get_status", json!({}))])]);
        let rig = rig(p, &server.base, &[]);
        rig.hear("m-1", "Mamie", "regarde");
        rig.drain();
        assert_eq!(server.count(), 10, "{p:?}");
        let last = &server.requests()[9];
        match p {
            Provider::Gemini => assert_eq!(last.body["toolConfig"]["functionCallingConfig"]["allowedFunctionNames"], json!(["reply", "show_card", "ignore"])),
            Provider::OpenAi => assert_eq!(last.body["tools"].as_array().unwrap().len(), 3),
        }
        assert_eq!(rig.turns()[0]["fin"], "sans_outil");
        assert_eq!(rig.room.texts(), vec![crate::strings::text("fr", "hermes.fallback.timeout", &[])]);
    });
}

#[test]
fn malformed_model_output_is_reported_back_and_the_turn_recovers() {
    let server = FakeModel::scripted(vec![
        Reply::Json(200, json!({ "choices": [{ "message": { "role": "assistant", "content": null, "tool_calls": [{ "id": "a", "type": "function", "function": { "name": "reply", "arguments": "{oops" } }] } }] })),
        step(Provider::OpenAi, &[("reply", json!({ "message": "Corrigé." }))]),
    ]);
    let rig = rig(Provider::OpenAi, &server.base, &[]);
    rig.hear("m-1", "Mamie", "Hermès salut");
    rig.drain();
    assert!(results(Provider::OpenAi, &server.requests()[1])[0]["erreur"].as_str().unwrap().contains("arguments illisibles"));
    assert_eq!(rig.room.texts(), vec!["Corrigé.".to_string()]);
    let server = FakeModel::scripted(vec![Reply::Raw(200, "not json at all".into())]);
    let rig = rig_for_error(&server);
    rig.hear("m-1", "Mamie", "Hermès salut");
    rig.drain();
    assert!(rig.room.texts().is_empty(), "an unreadable body is an error, not an answer");
    assert_eq!(rig.turns()[0]["fin"], "erreur");
}

fn rig_for_error(server: &FakeModel) -> Rig {
    rig(Provider::Gemini, &server.base, &[])
}

#[test]
fn a_timeout_posts_one_notice_and_the_log_names_the_cause() {
    both(|p| {
        let server = FakeModel::scripted(vec![Reply::Hang(Duration::from_secs(3))]);
        let rig = rig(p, &server.base, &[]);
        rig.hear("m-1", "Mamie", "mets tf1");
        rig.drain();
        assert_eq!(rig.room.texts(), vec![crate::strings::text("fr", "hermes.fallback.unavailable", &[])], "{p:?}");
        let errors: Vec<String> = rig.deps.store.audit_rows(Some("audit"), None, 10).unwrap().into_iter().filter(|r| r.event == "agent_error").map(|r| r.data["error"].as_str().unwrap().to_string()).collect();
        assert!(errors[0].contains("model call failed"), "{errors:?}");
        assert!(!format!("{errors:?}").contains(KEY));
    });
}

#[test]
fn a_rate_limit_is_noticed_once_and_never_retried_in_a_loop() {
    both(|p| {
        let server = FakeModel::scripted(vec![Reply::Json(429, json!({ "error": "quota" }))]);
        let rig = rig(p, &server.base, &[]);
        rig.hear("m-1", "Mamie", "mets tf1");
        rig.drain();
        rig.advance(5000);
        rig.hear("m-2", "Papi", "mets m6");
        rig.drain();
        assert_eq!(rig.room.texts().len(), 1, "{p:?}");
        assert_eq!(server.count(), 2, "one call per message, no retry on 429");
    });
}

#[test]
fn a_bad_key_stays_silent_in_the_room_and_never_appears_in_any_record() {
    both(|p| {
        let server = FakeModel::scripted(vec![Reply::Json(401, json!({ "error": { "message": format!("invalid key {KEY}") } }))]);
        let rig = rig(p, &server.base, &[]);
        rig.hear("m-1", "Mamie", "mets tf1");
        rig.drain();
        assert!(rig.room.texts().is_empty());
        let stored = format!("{:?}{:?}", rig.deps.store.audit_rows(None, None, 200).unwrap(), rig.room.said.lock().unwrap());
        assert!(!stored.contains(KEY), "{p:?}");
        assert!(stored.contains("HTTP 401"));
    });
}

#[test]
fn the_key_travels_only_in_the_header() {
    both(|p| {
        let server = FakeModel::scripted(vec![step(p, &[("reply", json!({ "message": "ok" }))])]);
        let rig = rig(p, &server.base, &[]);
        rig.hear("m-1", "Mamie", "Hermès ?");
        rig.drain();
        let req = &server.requests()[0];
        assert!(!req.path.contains(KEY) && !req.body.to_string().contains(KEY), "{p:?}");
        let header = match p {
            Provider::Gemini => req.headers["x-goog-api-key"].clone(),
            Provider::OpenAi => req.headers["authorization"].clone(),
        };
        assert!(header.contains(KEY));
    });
}

#[test]
fn the_test_and_list_calls_use_the_configured_provider() {
    both(|p| {
        let body = match p {
            Provider::Gemini => json!({ "candidates": [{ "content": { "parts": [{ "text": "ok" }] } }] }),
            Provider::OpenAi => json!({ "choices": [{ "message": { "content": "ok" } }] }),
        };
        let server = FakeModel::scripted(vec![Reply::Json(200, body)]);
        let rig = rig(p, &server.base, &[]);
        let model = rig.deps.model().unwrap();
        assert_eq!(model.ping().unwrap(), "ok", "{p:?}");
    });
}
