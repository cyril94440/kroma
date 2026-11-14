use std::sync::{Arc, Mutex};

use axum::routing::post;
use axum::{Json, Router};
use serde_json::{json, Value};

use kroma_module_sdk::host::testing::StubHost;

use super::*;

#[test]
fn a_status_reads_past_fields_this_module_ignores() {
    let status: Status = serde_json::from_value(json!({
        "state": "running",
        "channel": { "id": "c1", "name": "TF1", "invented": 1 },
        "provider": "backup",
        "now": { "title": "News", "start": 10, "end": 20 },
        "next": null,
        "viewers": [{ "userId": "u1", "name": "Ana" }],
        "health": { "sourceKbps": 4200.5, "stutter": true, "black": false, "frozen": false },
        "futureField": true
    }))
    .unwrap();

    assert_eq!(status.state, "running");
    assert_eq!(status.channel.unwrap().name, "TF1");
    assert_eq!(status.provider, "backup");
    assert_eq!(status.now.unwrap().end, 20);
    assert_eq!(status.viewers[0].user_id, "u1");
    assert!(status.health.stutter);
}

#[test]
fn an_empty_answer_defaults_rather_than_fails() {
    let status: Status = serde_json::from_value(json!({})).unwrap();
    assert!(status.channel.is_none() && status.viewers.is_empty());
    let switched: Switched = serde_json::from_value(json!({ "switched": false, "error": "offAir" })).unwrap();
    assert_eq!(switched.error.as_deref(), Some("offAir"));
}

type Seen = Arc<Mutex<Vec<(String, Value)>>>;

fn fake_live(seen: Seen) -> Router<()> {
    let record = |seen: Seen, method: &'static str, reply: Value| {
        move |Json(body): Json<Value>| {
            let seen = seen.clone();
            let reply = reply.clone();
            async move {
                seen.lock().unwrap().push((method.to_string(), body));
                Json(json!({ "Ok": reply }))
            }
        }
    };
    Router::new()
        .route(
            "/_port/tv.kroma.live/control/status",
            post(record(seen.clone(), "status", json!({ "state": "running", "channel": { "id": "c1", "name": "TF1" } }))),
        )
        .route(
            "/_port/tv.kroma.live/control/presence",
            post(record(seen.clone(), "presence", json!({ "viewers": [{ "userId": "u1", "name": "Ana", "since": 5 }] }))),
        )
        .route(
            "/_port/tv.kroma.live/control/switch",
            post(record(seen.clone(), "switch", json!({ "switched": true, "channel": { "id": "c2", "name": "M6" } }))),
        )
        .route(
            "/_port/tv.kroma.live/control/proposal",
            post(record(seen.clone(), "proposal", json!({ "applied": true }))),
        )
        .route(
            "/_port/tv.kroma.live/control/resolve",
            post(record(seen, "resolve", json!({ "channels": [{ "id": "c2", "name": "M6", "group": "FR", "score": 0.9 }] }))),
        )
}

#[tokio::test]
async fn every_call_reaches_the_control_point_under_the_keys_the_peer_parses() {
    let seen: Seen = Arc::default();
    let resolve = kroma_module_sdk::testing::serve(fake_live(seen.clone()), ()).await;
    let base = resolve().unwrap();
    let host = Arc::new(StubHost::new().with_point(CONTROL, None, &base.0, &base.1));
    let live = Arc::new(PeerLive::new(host));

    let runner = live.clone();
    let outcome = kroma_module_sdk::testing::blocking(move || {
        let status = runner.status().unwrap();
        let viewers = runner.presence().unwrap();
        let switched = runner.switch("c2", "u1", "vote").unwrap();
        let applied = runner.proposal("p1", true).unwrap();
        let found = runner.resolve("m6", 3).unwrap();
        (status, viewers, switched, applied, found)
    })
    .await;

    assert_eq!(outcome.0.channel.unwrap().id, "c1");
    assert_eq!(outcome.1[0].since, 5);
    assert!(outcome.2.switched);
    assert!(outcome.3);
    assert_eq!(outcome.4[0].name, "M6");

    let seen = seen.lock().unwrap();
    let body = |method: &str| seen.iter().find(|(m, _)| m == method).unwrap().1.clone();
    assert_eq!(body("status"), json!({}));
    assert_eq!(body("presence"), json!({}));
    assert_eq!(body("switch"), json!({ "channelId": "c2", "requestedBy": "u1", "reason": "vote" }));
    assert_eq!(body("proposal"), json!({ "id": "p1", "accept": true }));
    assert_eq!(body("resolve"), json!({ "query": "m6", "limit": 3 }));
}

#[test]
fn a_missing_peer_is_an_error_not_a_panic() {
    let live = PeerLive::new(Arc::new(StubHost::new()));
    assert!(live.status().is_err());
}
