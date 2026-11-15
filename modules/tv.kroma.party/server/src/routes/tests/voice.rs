use axum::http::StatusCode;
use serde_json::{json, Value};

use super::support::{app, App};

async fn join(app: &App, token: &str, device: &str) -> String {
    let (status, answer) = app
        .post("/voice/join", token, json!({ "deviceId": device }))
        .await;
    assert_eq!(status, StatusCode::OK);
    answer["peerId"].as_str().unwrap().to_string()
}

fn offer(from: &str, to: &str) -> Value {
    json!({ "from": from, "to": to, "session": "s1", "kind": "description", "description": { "type": "offer", "sdp": "v=0" } })
}

fn signals(app: &App) -> Vec<(String, Value)> {
    app.party
        .tap
        .lock()
        .unwrap()
        .iter()
        .filter(|(to, topic, _)| to.is_some() && topic == "party.voice.signal")
        .map(|(to, _, payload)| (to.clone().unwrap(), payload.clone()))
        .collect()
}

#[tokio::test]
async fn every_voice_route_refuses_a_caller_without_a_session() {
    let app = app();
    for (method, path) in [
        ("GET", "/voice/state"),
        ("GET", "/voice/ice"),
        ("POST", "/voice/join"),
        ("POST", "/voice/leave"),
        ("POST", "/voice/beat"),
        ("POST", "/voice/mute"),
        ("POST", "/voice/speaking"),
        ("POST", "/voice/signal"),
    ] {
        let (status, _) = app.send(method, path, None, Some(json!({}))).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {path}");
    }
}

#[tokio::test]
async fn two_accounts_join_see_each_other_and_a_late_reader_gets_the_same_roster() {
    let app = app();

    let mamie = join(&app, "mamie", "device-aaaa").await;
    let papi = join(&app, "papi", "device-bbbb").await;
    let (_, state) = app.get("/voice/state", "cyril").await;

    let ids: Vec<&str> = state["participants"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["peerId"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec![mamie.as_str(), papi.as_str()]);
    assert_eq!(state["participants"][1]["name"], "papi");
}

#[tokio::test]
async fn a_join_with_a_bad_device_id_is_a_bad_request() {
    let app = app();

    let (status, body) = app
        .post("/voice/join", "mamie", json!({ "deviceId": "x" }))
        .await;

    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("badDevice"))
    );
}

#[tokio::test]
async fn a_signal_travels_only_to_the_addressed_account() {
    let app = app();
    let mamie = join(&app, "mamie", "device-aaaa").await;
    let papi = join(&app, "papi", "device-bbbb").await;
    join(&app, "cyril", "device-cccc").await;

    let (status, answer) = app
        .post("/voice/signal", "mamie", offer(&mamie, &papi))
        .await;

    assert_eq!((status, answer["n"].clone()), (StatusCode::OK, json!(1)));
    let sent = signals(&app);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, "papi");
    assert_eq!(sent[0].1["from"], json!(mamie));
}

#[tokio::test]
async fn a_participant_cannot_send_as_another_peer_and_a_stranger_cannot_send_at_all() {
    let app = app();
    let mamie = join(&app, "mamie", "device-aaaa").await;
    let papi = join(&app, "papi", "device-bbbb").await;

    let (forged, _) = app
        .post("/voice/signal", "papi", offer(&mamie, &papi))
        .await;
    let (stranger, _) = app
        .post("/voice/signal", "cyril", offer(&mamie, &papi))
        .await;
    let (unjoined, body) = app
        .post("/voice/signal", "cyril", offer("ghost-1", &papi))
        .await;

    assert_eq!(forged, StatusCode::FORBIDDEN);
    assert_eq!(stranger, StatusCode::FORBIDDEN);
    assert_eq!(
        (unjoined, body["error"].as_str()),
        (StatusCode::NOT_FOUND, Some("notJoined"))
    );
    assert!(signals(&app).is_empty());
}

#[tokio::test]
async fn a_signal_to_a_peer_that_left_says_peer_gone() {
    let app = app();
    let mamie = join(&app, "mamie", "device-aaaa").await;
    let papi = join(&app, "papi", "device-bbbb").await;
    app.post("/voice/leave", "papi", json!({ "peerId": papi }))
        .await;

    let (status, body) = app
        .post("/voice/signal", "mamie", offer(&mamie, &papi))
        .await;

    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::NOT_FOUND, Some("peerGone"))
    );
}

#[tokio::test]
async fn an_oversized_description_is_refused_and_a_malformed_body_is_a_bad_request() {
    let app = app();
    let mamie = join(&app, "mamie", "device-aaaa").await;
    let papi = join(&app, "papi", "device-bbbb").await;
    let mut big = offer(&mamie, &papi);
    big["description"]["sdp"] = json!("a".repeat(17 * 1024));

    let (too_big, _) = app.post("/voice/signal", "mamie", big).await;
    let (malformed, _) = app
        .post("/voice/signal", "mamie", json!({ "from": 3 }))
        .await;

    assert_eq!(too_big, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(malformed, StatusCode::BAD_REQUEST);
    assert!(signals(&app).is_empty());
}

#[tokio::test]
async fn a_beat_keeps_a_peer_and_a_beat_for_an_unknown_peer_asks_for_a_rejoin() {
    let app = app();
    let mamie = join(&app, "mamie", "device-aaaa").await;

    let (ok, answer) = app
        .post("/voice/beat", "mamie", json!({ "peerId": mamie }))
        .await;
    let (gone, body) = app
        .post("/voice/beat", "mamie", json!({ "peerId": "ghost-1" }))
        .await;

    assert_eq!(ok, StatusCode::OK);
    assert!(answer["revision"].is_i64());
    assert_eq!(
        (gone, body["error"].as_str()),
        (StatusCode::NOT_FOUND, Some("notJoined"))
    );
}

#[tokio::test]
async fn mute_and_speaking_are_reflected_in_the_state() {
    let app = app();
    let mamie = join(&app, "mamie", "device-aaaa").await;
    let papi = join(&app, "papi", "device-bbbb").await;

    app.post(
        "/voice/mute",
        "mamie",
        json!({ "peerId": mamie, "muted": true }),
    )
    .await;
    app.post(
        "/voice/speaking",
        "papi",
        json!({ "peerId": papi, "speaking": true }),
    )
    .await;
    let (_, state) = app.get("/voice/state", "mamie").await;

    assert_eq!(state["participants"][0]["muted"], true);
    assert_eq!(state["speaking"], json!([papi]));
}

#[tokio::test]
async fn nobody_can_mute_or_remove_someone_elses_peer() {
    let app = app();
    let mamie = join(&app, "mamie", "device-aaaa").await;

    let (mute, _) = app
        .post(
            "/voice/mute",
            "papi",
            json!({ "peerId": mamie, "muted": true }),
        )
        .await;
    let (leave, _) = app
        .post("/voice/leave", "papi", json!({ "peerId": mamie }))
        .await;
    let (_, state) = app.get("/voice/state", "papi").await;

    assert_eq!(
        (mute, leave),
        (StatusCode::FORBIDDEN, StatusCode::FORBIDDEN)
    );
    assert_eq!(state["participants"][0]["muted"], false);
}

#[tokio::test]
async fn ice_servers_are_for_participants_only() {
    let app = app();
    join(&app, "mamie", "device-aaaa").await;

    let (member, answer) = app.get("/voice/ice", "mamie").await;
    let (stranger, _) = app.get("/voice/ice", "papi").await;

    assert_eq!(member, StatusCode::OK);
    assert_eq!(
        answer["iceServers"][0]["urls"],
        json!(["stun:stun.l.google.com:19302"])
    );
    assert_eq!(stranger, StatusCode::NOT_FOUND);
}
