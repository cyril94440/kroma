use std::time::{Duration, Instant};

use kroma_module_sdk::host::testing::StubHost;
use serde_json::{json, Value};

use super::fake::{FakeModel, Reply};
use crate::model::{Allowed, Model, ModelError, ToolCall, ToolDecl};
use crate::settings::Settings;

pub const KEY: &str = "AIza-test-key-0123";

pub fn settings(base: &str) -> Settings {
    let host = StubHost::new()
        .with_config("provider", json!("gemini"))
        .with_config("geminiApiKey", json!(KEY))
        .with_config("geminiModel", json!("gemini-test"))
        .with_config("geminiBaseUrl", json!(base))
        .with_config("modelTimeoutSeconds", json!(1));
    Settings::read(&host)
}

fn model(base: &str) -> std::sync::Arc<dyn Model> {
    crate::model::model_from(&settings(base)).expect("configured")
}

fn tools() -> Vec<ToolDecl> {
    vec![
        ToolDecl { name: "reply".into(), description: "say".into(), parameters: json!({ "type": "object", "properties": { "message": { "type": "string" } } }) },
        ToolDecl { name: "get_status".into(), description: "status".into(), parameters: json!({ "type": "object", "properties": {} }) },
    ]
}

fn call_reply(parts: Value) -> Reply {
    Reply::Json(200, json!({ "candidates": [{ "content": { "role": "model", "parts": parts }, "finishReason": "STOP" }] }))
}

#[test]
fn the_request_carries_the_documented_shape() {
    let server = FakeModel::scripted(vec![call_reply(json!([{ "functionCall": { "name": "reply", "args": { "message": "Salut" } } }]))]);
    let mut dialogue = model(&server.base).open("SYSTEM", "FIRST", &tools());
    let calls = dialogue.step(&Allowed::Any).unwrap();
    assert_eq!(calls[0].name, "reply");
    assert_eq!(calls[0].args, json!({ "message": "Salut" }));
    let req = &server.requests()[0];
    assert_eq!(req.method, "POST");
    assert_eq!(req.path, "/v1beta/models/gemini-test:generateContent");
    assert_eq!(req.headers["x-goog-api-key"], KEY);
    assert_eq!(req.body["systemInstruction"]["parts"][0]["text"], "SYSTEM");
    assert_eq!(req.body["contents"][0], json!({ "role": "user", "parts": [{ "text": "FIRST" }] }));
    assert_eq!(req.body["tools"][0]["functionDeclarations"][0]["name"], "reply");
    assert_eq!(req.body["toolConfig"]["functionCallingConfig"], json!({ "mode": "ANY" }));
    assert_eq!(req.body["generationConfig"]["temperature"], 0.0);
    assert_eq!(req.body["serviceTier"], "priority");
}

#[test]
fn a_restricted_step_names_the_allowed_functions() {
    let server = FakeModel::scripted(vec![call_reply(json!([{ "functionCall": { "name": "reply", "args": {} } }]))]);
    let mut dialogue = model(&server.base).open("S", "F", &tools());
    dialogue.step(&Allowed::Only(vec!["reply".into()])).unwrap();
    assert_eq!(server.requests()[0].body["toolConfig"]["functionCallingConfig"], json!({ "mode": "ANY", "allowedFunctionNames": ["reply"] }));
}

#[test]
fn results_go_back_as_function_responses_after_the_models_own_content() {
    let server = FakeModel::scripted(vec![
        call_reply(json!([{ "functionCall": { "id": "c1", "name": "get_status", "args": {} }, "thoughtSignature": "sig" }])),
        call_reply(json!([{ "functionCall": { "name": "reply", "args": { "message": "ok" } } }])),
    ]);
    let mut dialogue = model(&server.base).open("S", "F", &tools());
    let calls = dialogue.step(&Allowed::Any).unwrap();
    dialogue.answer(&[(calls[0].clone(), json!({ "chaine": "TF1" }))]);
    dialogue.step(&Allowed::Any).unwrap();
    let contents = &server.requests()[1].body["contents"];
    assert_eq!(contents[1]["role"], "model");
    assert_eq!(contents[1]["parts"][0]["thoughtSignature"], "sig");
    assert_eq!(contents[2]["role"], "user");
    assert_eq!(contents[2]["parts"][0]["functionResponse"], json!({ "id": "c1", "name": "get_status", "response": { "result": { "chaine": "TF1" } } }));
}

#[test]
fn a_call_without_an_id_is_answered_without_one() {
    let server = FakeModel::scripted(vec![call_reply(json!([{ "functionCall": { "name": "get_status", "args": {} } }])), call_reply(json!([]))]);
    let mut dialogue = model(&server.base).open("S", "F", &tools());
    let calls = dialogue.step(&Allowed::Any).unwrap();
    dialogue.answer(&[(calls[0].clone(), json!("x"))]);
    dialogue.step(&Allowed::Any).unwrap();
    let response = &server.requests()[1].body["contents"][2]["parts"][0]["functionResponse"];
    assert!(response.get("id").is_none());
    assert_eq!(response["name"], "get_status");
}

#[test]
fn text_instead_of_a_call_becomes_a_reply_and_nothing_becomes_no_call() {
    let server = FakeModel::scripted(vec![call_reply(json!([{ "text": " Bonsoir " }])), call_reply(json!([]))]);
    let mut dialogue = model(&server.base).open("S", "F", &tools());
    assert_eq!(dialogue.step(&Allowed::Any).unwrap(), vec![ToolCall::new("reply", json!({ "message": "Bonsoir" }))]);
    assert!(dialogue.step(&Allowed::Any).unwrap().is_empty());
}

#[test]
fn arguments_that_are_not_an_object_are_flagged() {
    let server = FakeModel::scripted(vec![call_reply(json!([{ "functionCall": { "name": "reply", "args": "oops" } }]))]);
    let mut dialogue = model(&server.base).open("S", "F", &tools());
    let calls = dialogue.step(&Allowed::Any).unwrap();
    assert!(calls[0].malformed.is_some());
    assert_eq!(calls[0].args, json!({}));
}

#[test]
fn an_answer_without_candidates_is_malformed_or_blocked() {
    let server = FakeModel::scripted(vec![Reply::Json(200, json!({ "usageMetadata": {} })), Reply::Json(200, json!({ "promptFeedback": { "blockReason": "SAFETY" } }))]);
    let mut dialogue = model(&server.base).open("S", "F", &tools());
    assert!(matches!(dialogue.step(&Allowed::Any), Err(ModelError::Malformed(_))));
    match dialogue.step(&Allowed::Any) {
        Err(ModelError::Rejected(m)) => assert!(m.contains("SAFETY")),
        other => panic!("{other:?}"),
    }
}

#[test]
fn rate_limiting_is_reported_without_a_retry() {
    let server = FakeModel::scripted(vec![Reply::Json(429, json!({ "error": "quota" }))]);
    let mut dialogue = model(&server.base).open("S", "F", &tools());
    assert_eq!(dialogue.step(&Allowed::Any), Err(ModelError::RateLimited));
    assert_eq!(server.count(), 1);
}

#[test]
fn a_server_error_is_retried_once() {
    let server = FakeModel::scripted(vec![Reply::Json(503, json!({})), call_reply(json!([{ "functionCall": { "name": "reply", "args": {} } }]))]);
    let mut dialogue = model(&server.base).open("S", "F", &tools());
    assert!(dialogue.step(&Allowed::Any).is_ok());
    assert_eq!(server.count(), 2);
    let failing = FakeModel::scripted(vec![Reply::Json(500, json!({}))]);
    let mut dialogue = model(&failing.base).open("S", "F", &tools());
    assert!(matches!(dialogue.step(&Allowed::Any), Err(ModelError::Transient(_))));
    assert_eq!(failing.count(), 2);
}

#[test]
fn a_rejection_never_echoes_the_key_back() {
    let server = FakeModel::scripted(vec![Reply::Json(400, json!({ "error": { "message": format!("API key not valid: {KEY}") } }))]);
    let mut dialogue = model(&server.base).open("S", "F", &tools());
    match dialogue.step(&Allowed::Any) {
        Err(ModelError::Rejected(m)) => {
            assert!(m.contains("HTTP 400"));
            assert!(!m.contains(KEY), "{m}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_body_that_is_not_json_is_malformed() {
    let server = FakeModel::scripted(vec![Reply::Raw(200, "<html>".into())]);
    let mut dialogue = model(&server.base).open("S", "F", &tools());
    assert!(matches!(dialogue.step(&Allowed::Any), Err(ModelError::Malformed(_))));
}

#[test]
fn a_silent_server_times_out_after_one_retry() {
    let server = FakeModel::scripted(vec![Reply::Hang(Duration::from_secs(4))]);
    let mut dialogue = model(&server.base).open("S", "F", &tools());
    let started = Instant::now();
    assert!(matches!(dialogue.step(&Allowed::Any), Err(ModelError::Transient(_))));
    assert!(started.elapsed() < Duration::from_secs(6));
    assert_eq!(server.count(), 2);
}

#[test]
fn the_model_list_keeps_what_can_generate_content() {
    let server = FakeModel::scripted(vec![Reply::Json(200, json!({ "models": [
        { "name": "models/gemini-a", "supportedGenerationMethods": ["generateContent"] },
        { "name": "models/embed", "supportedGenerationMethods": ["embedContent"] },
        { "name": "models/gemini-b" },
    ] }))]);
    assert_eq!(model(&server.base).models().unwrap(), vec!["gemini-a".to_string(), "gemini-b".to_string()]);
    let req = &server.requests()[0];
    assert_eq!(req.method, "GET");
    assert!(req.path.starts_with("/v1beta/models"));
    assert_eq!(req.headers["x-goog-api-key"], KEY);
}

#[test]
fn a_web_search_uses_grounding_and_keeps_domains_only() {
    let server = FakeModel::scripted(vec![Reply::Json(200, json!({ "candidates": [{
        "content": { "parts": [{ "text": "Lens joue à 21h" }] },
        "groundingMetadata": { "groundingChunks": [
            { "web": { "title": "lequipe.fr", "uri": "https://x/secret" } },
            { "web": { "title": "lequipe.fr" } },
            { "web": { "title": "uefa.com" } },
        ] },
    }] }))]);
    let answer = model(&server.base).web_search("quand joue Lens").unwrap();
    assert_eq!(answer.text, "Lens joue à 21h");
    assert_eq!(answer.sources, vec!["lequipe.fr".to_string(), "uefa.com".to_string()]);
    assert_eq!(server.requests()[0].body["tools"], json!([{ "googleSearch": {} }]));
}

#[test]
fn reading_a_screen_sends_the_image_inline_and_parses_the_json() {
    let server = FakeModel::scripted(vec![call_reply(json!([{ "text": "{\"type\":\"direct\",\"team1\":\"Lens\",\"confidence\":0.9}" }]))]);
    let reading = model(&server.base).read_screen("QUJD", "qui joue ?").unwrap();
    assert_eq!((reading.kind.as_str(), reading.team1.as_str()), ("direct", "Lens"));
    let body = &server.requests()[0].body;
    assert_eq!(body["contents"][0]["parts"][1]["inlineData"], json!({ "mimeType": "image/jpeg", "data": "QUJD" }));
    assert_eq!(body["generationConfig"]["responseMimeType"], "application/json");
}

#[test]
fn a_ping_returns_the_answer_text() {
    let server = FakeModel::scripted(vec![call_reply(json!([{ "text": "ok" }]))]);
    assert_eq!(model(&server.base).ping().unwrap(), "ok");
}

#[test]
fn a_model_name_with_the_resource_prefix_is_accepted() {
    let host = StubHost::new().with_config("geminiApiKey", json!(KEY)).with_config("geminiModel", json!("models/gemini-x")).with_config("geminiBaseUrl", json!("http://127.0.0.1:1"));
    let server = FakeModel::scripted(vec![call_reply(json!([]))]);
    let host = host.with_config("geminiBaseUrl", json!(server.base));
    let model = crate::model::model_from(&Settings::read(&host)).unwrap();
    model.ping().unwrap();
    assert_eq!(server.requests()[0].path, "/v1beta/models/gemini-x:generateContent");
}
