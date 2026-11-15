use serde_json::json;

use super::support::*;

#[tokio::test]
async fn say_answers_with_the_id_and_history_returns_the_pinned_message_shape() {
    let app = app();
    let said = app.port("say", json!({ "text": "Bonsoir", "as": "agent", "card": null, "replyTo": null })).await;
    let id = said["Ok"]["id"].as_str().unwrap().to_string();
    let history = app.port("history", json!({ "limit": 10, "before": null })).await;
    let m = &history["Ok"]["messages"][0];
    assert_eq!(m["id"], json!(id));
    assert_eq!(m["kind"], "agent");
    assert_eq!(m["text"], "Bonsoir");
    assert!(m["at"].is_i64());
    let mut keys: Vec<&str> = m.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, vec!["at", "id", "kind", "name", "text", "userId"]);
    let system = app.port("say", json!({ "text": "Maintenance", "as": "system" })).await;
    assert!(system["Ok"]["id"].is_string());
    let empty = app.port("say", json!({ "text": " ", "as": "agent" })).await;
    assert_eq!(empty["Err"], "empty");
}

#[tokio::test]
async fn request_switch_answers_each_outcome_of_the_contract() {
    let app = app();
    app.watching(&["mamie"]);
    let alone = app.port("requestSwitch", json!({ "channelId": "tf1", "requestedBy": "mamie", "reason": "asked" })).await;
    assert_eq!(alone["Ok"], json!({ "outcome": "switchedDirectly", "voteId": null, "reason": null }));
    app.watching(&["mamie", "papi"]);
    let started = app.port("requestSwitch", json!({ "channelId": "m6", "requestedBy": "agent", "reason": "football" })).await;
    assert_eq!(started["Ok"]["outcome"], "voteStarted");
    assert!(started["Ok"]["voteId"].is_string());
    let busy = app.port("requestSwitch", json!({ "channelId": "arte", "requestedBy": "mamie", "reason": "" })).await;
    assert_eq!(busy["Ok"]["outcome"], "refused");
    assert_eq!(busy["Ok"]["reason"], "busy");
}

#[tokio::test]
async fn polls_and_kick_and_activity_answer_through_the_point() {
    let app = app();
    app.watching(&["mamie", "papi"]);
    let started = app.port("startPoll", json!({ "question": "Qui ?", "kind": "choice", "options": ["A", "B"], "durationS": 60 })).await;
    assert_eq!(started["Ok"]["poll"]["options"], json!(["A", "B"]));
    let voted = app.port("votePoll", json!({ "userId": "mamie", "value": "a" })).await;
    assert_eq!(voted["Ok"], json!({ "ok": true }));
    let absent = app.port("votePoll", json!({ "userId": "ghost", "value": "a" })).await;
    assert_eq!(absent["Err"], "notPresent");
    let closed = app.port("closePoll", json!({})).await;
    assert_eq!(closed["Ok"]["results"]["total"], 1);
    assert_eq!(app.port("closePoll", json!({})).await["Err"], "noActivePoll");

    app.post("/sync/beat", "mamie", json!({ "delay": 2.0 })).await;
    let kicked = app.port("kick", json!({ "userId": "mamie", "requestedBy": "agent", "reason": "rude" })).await;
    assert_eq!(kicked["Ok"]["outcome"], "kicked");
    let refused = app.port("kick", json!({ "userId": "mamie", "requestedBy": "agent", "reason": "again" })).await;
    assert_eq!(refused["Ok"]["outcome"], "refused");
    assert_eq!(refused["Ok"]["reason"], "notInParty");

    assert_eq!(app.port("activity", json!({ "status": "Searching" })).await["Ok"], json!({ "ok": true }));
    let (_, state) = app.get("/state", "mamie").await;
    assert_eq!(state["activity"], "Searching");
    app.port("activity", json!({ "status": null })).await;
    let (_, state) = app.get("/state", "mamie").await;
    assert!(state["activity"].is_null());
}

#[test]
fn the_manifest_declares_exactly_the_methods_the_point_answers() {
    let manifest: serde_json::Value = serde_json::from_str(include_str!("../../../../module.json")).unwrap();
    let methods: Vec<String> = manifest["definesPoints"][0]["methods"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m.as_str().unwrap().to_string())
        .collect();
    assert_eq!(manifest["definesPoints"][0]["name"], "room");
    for fixed in ["say", "history", "requestSwitch", "startPoll", "closePoll", "kick", "activity"] {
        assert!(methods.contains(&fixed.to_string()), "{fixed}");
    }
    assert_eq!(manifest["consumes"][0]["point"], "tv.kroma.live/control");
    assert_eq!(manifest["contributes"][0]["point"], "tv.kroma.party/room");
}

#[tokio::test]
async fn go_live_and_fine_sync_answer_like_the_admin_routes() {
    let app = app();
    let none = app.port("goLive", json!({ "requestedBy": "cyril" })).await;
    assert_eq!(none["Err"], "noActiveParty");
    let (code, _) = app.post("/sync/beat", "mamie", json!({ "delay": 3.0 })).await;
    assert_eq!(code, axum::http::StatusCode::OK);
    let (_, _) = app.post("/sync/beat", "papi", json!({ "delay": 5.0 })).await;
    let fine = app.port("fineSync", json!({ "requestedBy": "cyril" })).await;
    assert_eq!(fine["Ok"], json!({ "ok": true }));
    let live = app.port("goLive", json!({ "requestedBy": "cyril" })).await;
    assert_eq!(live["Ok"], json!({ "members": 2 }));
}

#[tokio::test]
async fn fine_sync_needs_two_measurable_members() {
    let app = app();
    let (_, _) = app.post("/sync/beat", "mamie", json!({ "delay": 3.0 })).await;
    let fine = app.port("fineSync", json!({})).await;
    assert_eq!(fine["Err"], "needTwoMembers");
}

#[tokio::test]
async fn client_logs_return_the_entries_the_admin_route_returns() {
    let app = app();
    let (code, _) = app
        .post("/clientlog", "mamie", json!({ "events": [{ "category": "sync", "event": "recalage", "data": { "gap": 2 } }] }))
        .await;
    assert_eq!(code, axum::http::StatusCode::OK);
    let logs = app.port("clientLogs", json!({ "userId": null, "limit": 10 })).await;
    let admin = app.get("/clientlog?limit=10", "cyril").await.1;
    assert_eq!(logs["Ok"]["events"], admin["events"]);
    let mine = app.port("clientLogs", json!({ "userId": "mamie", "limit": 10 })).await;
    assert_eq!(mine["Ok"]["events"][0]["event"], "recalage");
    let other = app.port("clientLogs", json!({ "userId": "papi", "limit": 10 })).await;
    assert_eq!(other["Ok"]["events"], json!([]));
}

#[tokio::test]
async fn force_is_honoured_only_for_a_bypass_user_and_otherwise_asks_for_a_vote() {
    let app = app();
    app.watching(&["mamie", "papi"]);
    let denied = app.port("requestSwitch", json!({ "channelId": "m6", "requestedBy": "mamie", "reason": "", "force": true })).await;
    assert_eq!(denied["Ok"]["outcome"], "voteStarted");
}

#[tokio::test]
async fn a_request_names_the_channel_it_was_given_when_live_cannot_resolve_the_id() {
    let app = app();
    app.watching(&["mamie", "papi"]);
    app.port("requestSwitch", json!({ "channelId": "p:3", "channelName": "TF1", "requestedBy": "mamie", "reason": "" })).await;
    let (_, state) = app.get("/state", "papi").await;
    assert_eq!(state["vote"]["label"], "TF1");
}
