use axum::http::StatusCode;
use serde_json::json;

use super::support::*;

#[tokio::test]
async fn every_route_refuses_a_caller_without_a_session() {
    let app = app();
    for (method, path) in [("GET", "/state"), ("POST", "/chat"), ("POST", "/vote"), ("GET", "/ideas"), ("GET", "/time")] {
        let (status, _) = app.send(method, path, None, Some(json!({}))).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {path}");
    }
    let (status, _) = app.send("GET", "/state", Some("forged"), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn two_sessions_chat_and_a_joining_client_reads_the_history_from_the_state() {
    let app = app();
    let (status, sent) = app.post("/chat", "mamie", json!({ "text": "Bonsoir" })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(sent["message"]["name"], "mamie");
    app.post("/chat", "papi", json!({ "text": "Salut" })).await;
    let (_, state) = app.get("/state", "mamie").await;
    let texts: Vec<&str> = state["messages"].as_array().unwrap().iter().map(|m| m["text"].as_str().unwrap()).collect();
    assert_eq!(texts, vec!["Bonsoir", "Salut"]);
    assert_eq!(state["me"], json!({ "userId": "mamie", "name": "mamie", "admin": false }));
    let (_, history) = app.get("/chat/history?limit=1", "mamie").await;
    assert_eq!(history["messages"].as_array().unwrap().len(), 1);
    let (_, state) = app.get("/state", "cyril").await;
    assert_eq!(state["me"]["admin"], true);
}

#[tokio::test]
async fn an_empty_message_is_a_bad_request_and_a_flood_is_throttled() {
    let app = app();
    let (status, body) = app.post("/chat", "mamie", json!({ "text": "  " })).await;
    assert_eq!((status, body["error"].as_str()), (StatusCode::BAD_REQUEST, Some("empty")));
    for _ in 0..8 {
        app.post("/chat", "mamie", json!({ "text": "x" })).await;
    }
    let (status, body) = app.post("/chat", "mamie", json!({ "text": "x" })).await;
    assert_eq!((status, body["error"].as_str()), (StatusCode::TOO_MANY_REQUESTS, Some("rateLimited")));
}

#[tokio::test]
async fn a_channel_vote_that_passes_and_one_that_fails_run_over_http() {
    let app = app();
    app.watching(&["mamie", "papi", "cyril"]);
    let (_, started) = app.post("/request-switch", "mamie", json!({ "channelId": "tf1", "channelName": "TF1" })).await;
    assert_eq!(started["outcome"], "voteStarted");
    let (_, state) = app.get("/state", "papi").await;
    assert_eq!(state["vote"]["label"], "TF1");
    assert_eq!(state["vote"]["by"]["name"], "mamie");
    let (status, _) = app.post("/vote", "mamie", json!({ "approve": true })).await;
    assert_eq!(status, StatusCode::CONFLICT);
    app.post("/vote", "papi", json!({ "approve": true })).await;
    let (_, state) = app.get("/state", "papi").await;
    assert!(state["vote"].is_null());
    assert_eq!(*app.live.switched.lock().unwrap(), vec!["tf1"]);

    let (_, second) = app.post("/request-switch", "papi", json!({ "channelId": "m6" })).await;
    assert_eq!(second["outcome"], "voteStarted");
    app.post("/vote", "mamie", json!({ "approve": false })).await;
    app.post("/vote", "cyril", json!({ "approve": false })).await;
    assert_eq!(*app.live.switched.lock().unwrap(), vec!["tf1"]);
    let (_, votes) = app.get("/votes", "cyril").await;
    assert_eq!(votes["votes"].as_array().unwrap().len(), 2);
    assert_eq!(votes["votes"][0]["verdict"], "refused");
}

#[tokio::test]
async fn forcing_a_switch_is_honoured_for_an_administrator_and_flagged_for_anyone_else() {
    let app = app();
    app.watching(&["mamie", "cyril"]);
    let (_, denied) = app.post("/request-switch", "mamie", json!({ "channelId": "tf1", "force": true })).await;
    assert_eq!(denied["outcome"], "voteStarted");
    assert_eq!(denied["forceDenied"], true);
    app.party.cast_vote(&crate::model::Person::new("cyril", "cyril"), false).unwrap();
    let (_, forced) = app.post("/request-switch", "cyril", json!({ "channelId": "m6", "force": true })).await;
    assert_eq!(forced["outcome"], "switchedDirectly");
    let (status, _) = app.post("/request-switch", "cyril", json!({ "channelId": " " })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn polls_are_started_by_an_administrator_and_answered_by_everyone() {
    let app = app();
    app.watching(&["mamie", "papi"]);
    let (status, _) = app.post("/poll/start", "cyril", json!({ "question": "Match ?", "kind": "yesno" })).await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = app.post("/poll/start", "cyril", json!({ "question": "Again", "kind": "yesno" })).await;
    assert_eq!((status, body["error"].as_str()), (StatusCode::BAD_REQUEST, Some("pollAlreadyActive")));
    let (_, poll) = app.get("/poll", "mamie").await;
    assert_eq!(poll["poll"]["question"], "Match ?");
    let (status, _) = app.post("/poll/vote", "mamie", json!({ "value": "maybe" })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    app.post("/poll/vote", "mamie", json!({ "value": "oui" })).await;
    app.post("/poll/vote", "papi", json!({ "value": "non" })).await;
    let (_, poll) = app.get("/poll", "mamie").await;
    assert!(poll["poll"].is_null());
    assert_eq!(poll["lastPoll"]["total"], 2);
    let (status, _) = app.post("/poll/close", "cyril", json!({})).await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn reactions_toggle_over_http_and_come_back_in_the_state() {
    let app = app();
    let (_, sent) = app.post("/chat", "mamie", json!({ "text": "but" })).await;
    let id = sent["message"]["id"].as_str().unwrap().to_string();
    let goal = crate::chat_rules::REACTION_EMOJIS[0];
    let (status, _) = app.post("/react", "papi", json!({ "id": id, "emoji": goal })).await;
    assert_eq!(status, StatusCode::OK);
    let (_, state) = app.get("/state", "mamie").await;
    assert_eq!(state["messages"][0]["reactions"][goal][0]["name"], "papi");
    let (status, _) = app.post("/react", "papi", json!({ "id": id, "emoji": "x" })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = app.post("/reaction", "papi", json!({ "emoji": goal })).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn ideas_are_public_triaged_by_an_administrator_and_answered() {
    let app = app();
    let (status, created) = app.post("/ideas", "mamie", json!({ "genre": "bug", "text": "no sound", "device": "iPhone" })).await;
    assert_eq!(status, StatusCode::OK);
    let id = created["id"].as_str().unwrap().to_string();
    let (status, body) = app.post("/ideas", "mamie", json!({ "text": "again" })).await;
    assert_eq!((status, body["error"].as_str()), (StatusCode::TOO_MANY_REQUESTS, Some("tooFast")));
    let (_, list) = app.get("/ideas", "papi").await;
    assert_eq!(list["ideas"][0]["author"]["name"], "mamie");
    assert!(list["ideas"][0].get("context").is_none());
    let (_, triage) = app.get("/ideas/triage?status=received", "cyril").await;
    assert_eq!(triage["ideas"][0]["context"]["device"], "iPhone");
    let (status, updated) = app.post(&format!("/ideas/{id}"), "cyril", json!({ "status": "done", "reply": "Fixed" })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["idea"]["status"], "done");
    let (status, _) = app.post(&format!("/ideas/{id}"), "cyril", json!({ "status": "bogus" })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = app.post("/ideas/ghost", "cyril", json!({ "status": "done" })).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_sync_routes_follow_the_source_semantics() {
    let app = app();
    app.watching(&["mamie", "papi"]);
    app.post("/sync/beat", "mamie", json!({ "delay": 3.0 })).await;
    app.post("/sync/beat", "papi", json!({ "delay": 5.0 })).await;
    let (_, info) = app.get("/sync/info", "cyril").await;
    assert_eq!(info["active"], true);
    assert_eq!(info["members"]["papi"]["delayS"], 5.0);
    let (status, kicked) = app.post("/sync/kick", "cyril", json!({ "userId": "papi", "reason": "x" })).await;
    assert_eq!((status, kicked["kicked"].as_bool()), (StatusCode::OK, Some(true)));
    let (status, body) = app.post("/sync/kick", "cyril", json!({ "userId": "ghost" })).await;
    assert_eq!((status, body["error"].as_str()), (StatusCode::CONFLICT, Some("notInParty")));
    app.post("/sync/beat", "mamie", json!({ "enabled": false })).await;
    let (status, body) = app.post("/sync/go-live", "cyril", json!({})).await;
    assert_eq!((status, body["error"].as_str()), (StatusCode::CONFLICT, Some("noActiveParty")));
    let (status, _) = app.post("/sync/recalibration", "mamie", json!({ "gap": 1.0 })).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn the_time_route_gives_the_server_clock_for_offset_estimation() {
    let app = app();
    let before = app.party.time_now();
    let (_, t) = app.get("/time", "mamie").await;
    let now = t["now"].as_i64().unwrap();
    assert!(now >= before && now - before < 5000);
}

#[tokio::test]
async fn viewer_stats_client_logs_and_health_are_collected_and_read_by_an_administrator() {
    let app = app();
    app.watching(&["mamie"]);
    app.post("/stats", "mamie", json!({ "buffer": 4.5, "bandwidthKbps": 6000, "stalls": 1, "playing": true, "resolution": "1080p" })).await;
    let (_, state) = app.get("/state", "mamie").await;
    assert_eq!(state["stats"]["mamie"]["bufferS"], 4.5);
    assert_eq!(state["stats"]["mamie"]["resolution"], "1080p");
    let (status, _) = app
        .post("/clientlog", "mamie", json!({ "device": "iPhone", "events": [{ "category": "player", "event": "stall", "data": { "ms": 800 } }, { "event": "" }] }))
        .await;
    assert_eq!(status, StatusCode::OK);
    let (_, logs) = app.get("/clientlog?category=player", "cyril").await;
    assert_eq!(logs["events"].as_array().unwrap().len(), 1);
    assert_eq!(logs["events"][0]["userId"], "mamie");
    let (_, health) = app.get("/health", "cyril").await;
    assert_eq!(health["ok"], true);
}

#[tokio::test]
async fn the_channel_search_goes_through_the_live_module() {
    let app = app();
    let (status, body) = app.get("/channels?q=tf1", "mamie").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["channels"], json!([]));
    let (_, blank) = app.get("/channels?q=%20", "mamie").await;
    assert_eq!(blank["channels"], json!([]));
}
