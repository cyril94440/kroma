use kroma_module_sdk::host::testing::StubHost;
use serde_json::{json, Value};

use super::fake::{FakeModel, Reply};
use crate::model::{Allowed, Model, ModelError, ToolDecl};
use crate::settings::Settings;

const KEY: &str = "sk-test-key-4567";

fn settings(base: &str, key: &str) -> Settings {
    let host = StubHost::new()
        .with_config("provider", json!("openai"))
        .with_config("openaiApiKey", json!(key))
        .with_config("openaiModel", json!("local-model"))
        .with_config("openaiBaseUrl", json!(base))
        .with_config("modelTimeoutSeconds", json!(1));
    Settings::read(&host)
}

fn model(base: &str) -> std::sync::Arc<dyn Model> {
    crate::model::model_from(&settings(base, KEY)).expect("configured")
}

fn tools() -> Vec<ToolDecl> {
    vec![
        ToolDecl { name: "reply".into(), description: "say".into(), parameters: json!({ "type": "object", "properties": { "message": { "type": "string" } } }) },
        ToolDecl { name: "get_status".into(), description: "status".into(), parameters: json!({ "type": "object", "properties": {} }) },
    ]
}

fn tool_reply(calls: Value) -> Reply {
    Reply::Json(200, json!({ "choices": [{ "index": 0, "finish_reason": "tool_calls", "message": { "role": "assistant", "content": null, "tool_calls": calls } }] }))
}

fn call(id: &str, name: &str, arguments: &str) -> Value {
    json!({ "id": id, "type": "function", "function": { "name": name, "arguments": arguments } })
}

#[test]
fn the_request_carries_the_documented_shape() {
    let server = FakeModel::scripted(vec![tool_reply(json!([call("a", "reply", "{\"message\":\"Salut\"}")]))]);
    let mut dialogue = model(&server.base).open("SYSTEM", "FIRST", &tools());
    let calls = dialogue.step(&Allowed::Any).unwrap();
    assert_eq!((calls[0].id.as_str(), calls[0].name.as_str()), ("a", "reply"));
    assert_eq!(calls[0].args, json!({ "message": "Salut" }));
    let req = &server.requests()[0];
    assert_eq!(req.path, "/chat/completions");
    assert_eq!(req.headers["authorization"], format!("Bearer {KEY}"));
    assert_eq!(req.body["model"], "local-model");
    assert_eq!(req.body["messages"][0], json!({ "role": "system", "content": "SYSTEM" }));
    assert_eq!(req.body["messages"][1], json!({ "role": "user", "content": "FIRST" }));
    assert_eq!(req.body["tools"][0], json!({ "type": "function", "function": { "name": "reply", "description": "say", "parameters": tools()[0].parameters } }));
    assert_eq!(req.body["tool_choice"], "required");
    assert_eq!(req.body["temperature"], 0.0);
}

#[test]
fn a_restricted_step_only_declares_the_allowed_tools() {
    let server = FakeModel::scripted(vec![tool_reply(json!([call("a", "reply", "{}")]))]);
    let mut dialogue = model(&server.base).open("S", "F", &tools());
    dialogue.step(&Allowed::Only(vec!["reply".into()])).unwrap();
    let declared = server.requests()[0].body["tools"].as_array().unwrap().len();
    assert_eq!(declared, 1);
}

#[test]
fn every_call_gets_its_own_tool_message() {
    let server = FakeModel::scripted(vec![
        tool_reply(json!([call("a", "get_status", "{}"), call("b", "reply", "{\"message\":\"x\"}")])),
        tool_reply(json!([call("c", "reply", "{}")])),
    ]);
    let mut dialogue = model(&server.base).open("S", "F", &tools());
    let calls = dialogue.step(&Allowed::Any).unwrap();
    dialogue.answer(&[(calls[0].clone(), json!({ "chaine": "TF1" })), (calls[1].clone(), json!("ok"))]);
    dialogue.step(&Allowed::Any).unwrap();
    let messages = &server.requests()[1].body["messages"];
    assert_eq!(messages[2]["role"], "assistant");
    assert_eq!(messages[3], json!({ "role": "tool", "tool_call_id": "a", "content": "{\"chaine\":\"TF1\"}" }));
    assert_eq!(messages[4], json!({ "role": "tool", "tool_call_id": "b", "content": "\"ok\"" }));
}

#[test]
fn arguments_that_are_not_valid_json_are_flagged_not_dropped() {
    let server = FakeModel::scripted(vec![tool_reply(json!([call("a", "reply", "{not json"), call("b", "reply", "[1]")]))]);
    let mut dialogue = model(&server.base).open("S", "F", &tools());
    let calls = dialogue.step(&Allowed::Any).unwrap();
    assert!(calls[0].malformed.as_deref().unwrap().contains("valid JSON"));
    assert!(calls[1].malformed.is_some());
}

#[test]
fn plain_content_becomes_a_reply() {
    let server = FakeModel::scripted(vec![Reply::Json(200, json!({ "choices": [{ "message": { "role": "assistant", "content": "Bonsoir" } }] }))]);
    let mut dialogue = model(&server.base).open("S", "F", &tools());
    assert_eq!(dialogue.step(&Allowed::Any).unwrap()[0].args, json!({ "message": "Bonsoir" }));
}

#[test]
fn an_answer_without_a_choice_is_malformed() {
    let server = FakeModel::scripted(vec![Reply::Json(200, json!({ "choices": [] }))]);
    let mut dialogue = model(&server.base).open("S", "F", &tools());
    assert!(matches!(dialogue.step(&Allowed::Any), Err(ModelError::Malformed(_))));
}

#[test]
fn errors_are_classified_and_never_echo_the_key() {
    let limited = FakeModel::scripted(vec![Reply::Json(429, json!({}))]);
    assert_eq!(model(&limited.base).open("S", "F", &tools()).step(&Allowed::Any), Err(ModelError::RateLimited));
    let flaky = FakeModel::scripted(vec![Reply::Json(502, json!({})), tool_reply(json!([call("a", "reply", "{}")]))]);
    assert!(model(&flaky.base).open("S", "F", &tools()).step(&Allowed::Any).is_ok());
    assert_eq!(flaky.count(), 2);
    let rejected = FakeModel::scripted(vec![Reply::Json(401, json!({ "error": { "message": format!("Incorrect API key provided: {KEY}") } }))]);
    match model(&rejected.base).open("S", "F", &tools()).step(&Allowed::Any) {
        Err(ModelError::Rejected(m)) => assert!(!m.contains(KEY), "{m}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_keyless_local_server_gets_no_authorization_header() {
    let server = FakeModel::scripted(vec![Reply::Json(200, json!({ "choices": [{ "message": { "content": "ok" } }] }))]);
    let model = crate::model::model_from(&settings(&server.base, "")).unwrap();
    assert_eq!(model.ping().unwrap(), "ok");
    assert!(!server.requests()[0].headers.contains_key("authorization"));
}

#[test]
fn models_come_from_the_data_list() {
    let server = FakeModel::scripted(vec![Reply::Json(200, json!({ "data": [{ "id": "m1" }, { "id": "m2" }] }))]);
    assert_eq!(model(&server.base).models().unwrap(), vec!["m1".to_string(), "m2".to_string()]);
    assert_eq!(server.requests()[0].path, "/models");
}

#[test]
fn web_search_is_not_offered_and_reading_a_screen_sends_a_data_uri() {
    let server = FakeModel::scripted(vec![Reply::Json(200, json!({ "choices": [{ "message": { "content": "{\"type\":\"noir\",\"confidence\":0.9}" } }] }))]);
    let model = model(&server.base);
    assert!(!model.can_search());
    assert!(matches!(model.web_search("x"), Err(ModelError::Rejected(_))));
    assert_eq!(model.read_screen("QUJD", "").unwrap().kind, "noir");
    assert_eq!(server.requests()[0].body["messages"][0]["content"][1]["image_url"]["url"], "data:image/jpeg;base64,QUJD");
}

#[test]
fn an_unconfigured_provider_builds_no_model() {
    let host = StubHost::new().with_config("provider", json!("gemini"));
    assert!(crate::model::model_from(&Settings::read(&host)).is_none());
    let host = StubHost::new().with_config("provider", json!("openai"));
    assert!(crate::model::model_from(&Settings::read(&host)).is_none());
}
