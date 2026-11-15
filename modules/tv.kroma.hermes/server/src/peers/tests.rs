use std::sync::Arc;

use axum::routing::post;
use axum::{Json, Router};
use kroma_module_sdk::host::testing::StubHost;
use kroma_module_sdk::host::HostCtx;
use kroma_module_sdk::testing::{blocking, serve};
use serde_json::{json, Value};

use super::*;

type Seen = Arc<std::sync::Mutex<Vec<(String, Value)>>>;

fn host_for(point: &str, resolver: &kroma_module_sdk::host::Resolver) -> Arc<dyn HostCtx> {
    let (base, token) = resolver().unwrap();
    Arc::new(StubHost::new().with_point(point, None, &base, &token))
}

async fn answer(point: &'static str, replies: Vec<(&'static str, Value)>) -> (Arc<dyn HostCtx>, Seen) {
    let seen: Seen = Arc::default();
    let mut router: Router<()> = Router::new();
    for (method, reply) in replies {
        let log = seen.clone();
        let route = format!("/_port/{point}/{method}");
        router = router.route(
            &route,
            post(move |Json(body): Json<Value>| {
                let log = log.clone();
                let reply = reply.clone();
                async move {
                    log.lock().unwrap().push((method.to_string(), body));
                    Json(Ok::<Value, String>(reply))
                }
            }),
        );
    }
    let resolver = serve(router, ()).await;
    (host_for(point, &resolver), seen)
}

#[tokio::test]
async fn the_room_calls_send_the_pinned_json_and_read_the_pinned_replies() {
    let (host, seen) = answer(
        ROOM,
        vec![
            ("say", json!({ "id": "m1" })),
            ("history", json!({ "messages": [{ "id": "a", "userId": "u1", "name": "Ana", "kind": "user", "text": "salut", "at": 7, "extra": true }] })),
            ("requestSwitch", json!({ "outcome": "voteStarted", "voteId": "3", "reason": null })),
            ("startPoll", json!({ "poll": { "question": "Q" } })),
            ("closePoll", json!({ "results": { "total": 1 } })),
            ("votePoll", json!({ "ok": true })),
            ("kick", json!({ "outcome": "refused", "reason": "notInParty", "disruptions": 0 })),
            ("activity", json!({ "ok": true })),
        ],
    )
    .await;
    let room = PeerRoom::new(host);
    let out = blocking(move || {
        let said = room.say("Bonsoir", Some(json!({ "kind": "match" })), Some("m0")).unwrap();
        let history = room.history(5, Some(9)).unwrap();
        let switched = room.request_switch(&SwitchAsk { channel_id: "tf1", channel_name: None, requested_by: "u1", reason: "foot", force: false }).unwrap();
        let poll = room.start_poll("u1", "Q", "choice", &["A".into(), "B".into()], Some(60)).unwrap();
        let closed = room.close_poll("u1").unwrap();
        room.vote_poll("u1", "a").unwrap();
        let kicked = room.kick("u2", "u1", "rude").unwrap();
        room.activity(Some("réfléchit")).unwrap();
        (said, history, switched, poll, closed, kicked)
    })
    .await;
    assert_eq!(out.0, "m1");
    assert_eq!(out.1[0], RoomMessage { id: "a".into(), user_id: Some("u1".into()), name: "Ana".into(), kind: "user".into(), text: "salut".into(), at: 7 });
    assert_eq!(out.2, SwitchOutcome { outcome: "voteStarted".into(), vote_id: Some("3".into()), reason: None });
    assert_eq!(out.3["question"], "Q");
    assert_eq!(out.4["total"], 1);
    assert_eq!(out.5, KickOutcome { outcome: "refused".into(), reason: Some("notInParty".into()), disruptions: 0 });
    let sent: std::collections::BTreeMap<String, Value> = seen.lock().unwrap().iter().cloned().collect();
    assert_eq!(sent["say"], json!({ "text": "Bonsoir", "as": "agent", "card": { "kind": "match" }, "replyTo": "m0" }));
    assert_eq!(sent["history"], json!({ "limit": 5, "before": 9 }));
    assert_eq!(sent["requestSwitch"], json!({ "channelId": "tf1", "requestedBy": "u1", "reason": "foot" }));
    assert_eq!(sent["startPoll"], json!({ "requestedBy": "u1", "question": "Q", "kind": "choice", "options": ["A", "B"], "durationS": 60 }));
    assert_eq!(sent["closePoll"], json!({ "requestedBy": "u1" }));
    assert_eq!(sent["votePoll"], json!({ "userId": "u1", "value": "a" }));
    assert_eq!(sent["kick"], json!({ "userId": "u2", "requestedBy": "u1", "reason": "rude" }));
    assert_eq!(sent["activity"], json!({ "status": "réfléchit" }));
}

#[tokio::test]
async fn the_control_calls_send_the_pinned_json_and_tolerate_missing_fields() {
    let (host, seen) = answer(
        CONTROL,
        vec![
            ("status", json!({ "state": "running", "channel": { "id": "tf1", "name": "TF1" }, "provider": "primary", "now": { "title": "J", "start": 1, "end": 2 }, "viewers": [{ "userId": "u1", "name": "Ana", "since": 5 }], "health": { "sourceKbps": 4000.0, "stutter": true } })),
            ("resolve", json!({ "channels": [{ "id": "m6", "name": "M6", "group": "FR", "score": 9.5, "now": null }] })),
            ("epg", json!({ "programmes": [{ "channelId": "m6", "channelName": "M6", "title": "Foot", "start": 1, "end": 2 }] })),
            ("snapshot", json!({ "jpegBase64": "QUJD", "at": 42 })),
        ],
    )
    .await;
    let control = PeerControl::new(host);
    let out = blocking(move || (control.status().unwrap(), control.resolve("m6", 3, None).unwrap(), control.epg(Some("foot"), Some(1), None, 10).unwrap(), control.snapshot().unwrap())).await;
    assert_eq!(out.0.channel.unwrap().name, "TF1");
    assert!(out.0.health.stutter && !out.0.health.black);
    assert_eq!(out.0.viewers[0].user_id, "u1");
    assert_eq!(out.1[0].id, "m6");
    assert_eq!(out.2[0].channel_name, "M6");
    assert_eq!(out.3, Snapshot { jpeg_base64: "QUJD".into(), at: 42 });
    let sent: std::collections::BTreeMap<String, Value> = seen.lock().unwrap().iter().cloned().collect();
    assert_eq!(sent["status"], json!({}));
    assert_eq!(sent["resolve"], json!({ "query": "m6", "limit": 3, "provider": null }));
    assert_eq!(sent["epg"], json!({ "query": "foot", "from": 1, "to": null, "limit": 10 }));
}

#[test]
fn a_missing_peer_is_an_error_and_a_missing_method_is_recognised() {
    let host: Arc<dyn HostCtx> = Arc::new(StubHost::new());
    assert!(PeerRoom::new(host.clone()).history(1, None).is_err());
    let error = PeerControl::new(host).status().unwrap_err();
    assert!(is_unsupported(&error));
    assert!(is_unsupported(&anyhow::anyhow!("HTTP 404: not found")));
    assert!(!is_unsupported(&anyhow::anyhow!("HTTP 500: boom")));
}

#[tokio::test]
async fn the_tools_point_lists_every_contributor_and_routes_calls_to_the_right_one() {
    let (host, seen) = answer(
        TOOLS,
        vec![
            ("list", json!({ "tools": [
                { "name": "match_facts", "description": "Faits d'un match", "parameters": { "type": "object", "properties": { "equipes": { "type": "string" } } } },
                { "name": "", "description": "ignored" },
                { "name": "no_schema", "description": "d", "parameters": "oops" },
            ] })),
            ("call", json!({ "result": { "score": "1-0" } })),
        ],
    )
    .await;
    let contributors = PeerContributors::new(host);
    let out = blocking(move || {
        let tools = contributors.tools(1_000);
        let cached = contributors.tools(2_000).len();
        let result = contributors.call("test.tv.kroma.hermes/tools", "match_facts", &json!({ "equipes": "Lens PSG" }), "u1", "m9");
        let missing = contributors.call("ghost", "x", &json!({}), "u1", "m9").is_err();
        (tools, cached, result.unwrap(), missing)
    })
    .await;
    assert_eq!(out.0.len(), 2);
    assert_eq!(out.0[0].decl.name, "match_facts");
    assert_eq!(out.0[1].decl.parameters, json!({ "type": "object", "properties": {} }));
    assert_eq!(out.1, 2);
    assert_eq!(out.2, json!({ "score": "1-0" }));
    assert!(out.3);
    let sent: Vec<(String, Value)> = seen.lock().unwrap().clone();
    assert_eq!(sent.iter().filter(|(m, _)| m == "list").count(), 1);
    let call = sent.iter().find(|(m, _)| m == "call").unwrap();
    assert_eq!(call.1, json!({ "name": "match_facts", "arguments": { "equipes": "Lens PSG" }, "context": { "userId": "u1", "messageId": "m9" } }));
}

#[test]
fn the_manifest_declares_the_peers_and_the_point_it_defines() {
    let manifest: Value = serde_json::from_str(include_str!("../../../module.json")).unwrap();
    let consumed: Vec<(&str, bool)> = manifest["consumes"].as_array().unwrap().iter().map(|c| (c["point"].as_str().unwrap(), c["optional"].as_bool().unwrap_or(false))).collect();
    assert!(consumed.contains(&(ROOM, false)) && consumed.contains(&(CONTROL, false)));
    assert_eq!(manifest["definesPoints"][0]["name"], "tools");
    assert_eq!(manifest["definesPoints"][0]["methods"], json!(["list", "call"]));
    assert_eq!(TOOLS, "tv.kroma.hermes/tools");
    assert_eq!(manifest["dependencies"], json!({ "tv.kroma.party": ">=0.1.0", "tv.kroma.live": ">=0.3.0" }));
}

#[test]
fn the_manifest_declares_exactly_the_methods_the_tools_point_is_called_with() {
    let manifest: Value = serde_json::from_str(include_str!("../../../module.json")).unwrap();
    let mut declared: Vec<String> = manifest["definesPoints"][0]["methods"].as_array().unwrap().iter().map(|m| m.as_str().unwrap().to_string()).collect();
    let mut called: Vec<String> = super::contributors::METHODS.iter().map(|m| m.to_string()).collect();
    declared.sort();
    called.sort();
    assert_eq!(manifest["definesPoints"][0]["name"], "tools");
    assert_eq!(declared, called);
}
