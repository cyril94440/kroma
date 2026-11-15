use axum::http::StatusCode;
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
    let started = app.port("startPoll", json!({ "requestedBy": "mamie", "question": "Qui ?", "kind": "choice", "options": ["A", "B"], "durationS": 60 })).await;
    assert_eq!(started["Ok"]["poll"]["options"], json!(["A", "B"]));
    let voted = app.port("votePoll", json!({ "userId": "mamie", "value": "a" })).await;
    assert_eq!(voted["Ok"], json!({ "ok": true }));
    let absent = app.port("votePoll", json!({ "userId": "ghost", "value": "a" })).await;
    assert_eq!(absent["Err"], "notPresent");
    let closed = app.port("closePoll", json!({ "requestedBy": "papi" })).await;
    assert_eq!(closed["Ok"]["results"]["total"], 1);
    assert_eq!(app.port("closePoll", json!({ "requestedBy": "papi" })).await["Err"], "noActivePoll");

    app.post("/sync/beat", "mamie", json!({ "delay": 2.0 })).await;
    let kicked = app.port("kick", json!({ "userId": "mamie", "requestedBy": "papi", "reason": "rude" })).await;
    assert_eq!(kicked["Ok"]["outcome"], "kicked");
    let refused = app.port("kick", json!({ "userId": "mamie", "requestedBy": "papi", "reason": "again" })).await;
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
    let mut declared = methods.clone();
    let mut answered: Vec<String> = include_str!("../../room.rs")
        .split(".route(&point(\"")
        .skip(1)
        .map(|rest| rest.split('"').next().unwrap().to_string())
        .collect();
    declared.sort();
    answered.sort();
    assert_eq!(declared, answered);
    assert_eq!(manifest["consumes"][0]["point"], "tv.kroma.live/control");
    assert_eq!(manifest["contributes"][0]["point"], "tv.kroma.party/room");
}

#[tokio::test]
async fn go_live_and_fine_sync_answer_any_present_viewer_like_the_admin_routes() {
    let app = app();
    app.watching(&["mamie", "papi"]);
    let none = app.port("goLive", json!({ "requestedBy": "mamie" })).await;
    assert_eq!(none["Err"], "noActiveParty");
    let (code, _) = app.post("/sync/beat", "mamie", json!({ "delay": 3.0 })).await;
    assert_eq!(code, axum::http::StatusCode::OK);
    let (_, _) = app.post("/sync/beat", "papi", json!({ "delay": 5.0 })).await;
    let fine = app.port("fineSync", json!({ "requestedBy": "papi" })).await;
    assert_eq!(fine["Ok"], json!({ "ok": true }));
    let live = app.port("goLive", json!({ "requestedBy": "mamie" })).await;
    assert_eq!(live["Ok"], json!({ "members": 2 }));
}

#[tokio::test]
async fn fine_sync_needs_two_measurable_members() {
    let app = app();
    app.watching(&["mamie"]);
    let (_, _) = app.post("/sync/beat", "mamie", json!({ "delay": 3.0 })).await;
    let fine = app.port("fineSync", json!({ "requestedBy": "mamie" })).await;
    assert_eq!(fine["Err"], "needTwoMembers");
}

#[tokio::test]
async fn client_logs_return_the_entries_the_admin_route_returns() {
    let app = app();
    app.seen_as_admin().await;
    app.watching(&["papi"]);
    let (code, _) = app
        .post("/clientlog", "mamie", json!({ "events": [{ "category": "sync", "event": "recalage", "data": { "gap": 2 } }] }))
        .await;
    assert_eq!(code, axum::http::StatusCode::OK);
    let logs = app.port("clientLogs", json!({ "requestedBy": "papi", "userId": null, "limit": 10 })).await;
    let admin = app.get("/clientlog?limit=10", "cyril").await.1;
    assert_eq!(logs["Ok"]["events"], admin["events"]);
    let mine = app.port("clientLogs", json!({ "requestedBy": "papi", "userId": "mamie", "limit": 10 })).await;
    assert_eq!(mine["Ok"]["events"][0]["event"], "recalage");
    let other = app.port("clientLogs", json!({ "requestedBy": "papi", "userId": "papi", "limit": 10 })).await;
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

#[tokio::test]
async fn the_agent_door_refuses_anyone_who_is_not_a_present_viewer() {
    let app = app();
    app.watching(&["mamie", "papi"]);
    app.post("/sync/beat", "mamie", json!({ "delay": 3.0 })).await;
    app.post("/sync/beat", "papi", json!({ "delay": 5.0 })).await;

    for who in ["agent", "ghost", ""] {
        let kick = app.port("kick", json!({ "userId": "papi", "requestedBy": who, "reason": "x" })).await;
        assert_eq!(kick["Ok"], json!({ "outcome": "refused", "reason": "notPresent", "disruptions": 0 }), "kick by {who:?}");
        let calls = [
            ("startPoll", json!({ "requestedBy": who, "question": "Qui ?", "kind": "yesno" })),
            ("closePoll", json!({ "requestedBy": who })),
            ("goLive", json!({ "requestedBy": who })),
            ("fineSync", json!({ "requestedBy": who })),
            ("clientLogs", json!({ "requestedBy": who, "limit": 5 })),
        ];
        for (method, body) in calls {
            assert_eq!(app.port(method, body).await["Err"], "notPresent", "{method} by {who:?}");
        }
    }

    assert_eq!(app.party.lock().sync.member_count(), 2);
}

#[tokio::test]
async fn a_member_kick_through_the_agent_door_follows_the_server_rules() {
    let app = app();
    app.watching(&["mamie", "papi", "ced"]);
    app.post("/sync/beat", "papi", json!({ "delay": 2.0 })).await;
    app.post("/sync/beat", "ced", json!({ "delay": 2.0 })).await;

    let absent = app.port("kick", json!({ "userId": "ghost", "requestedBy": "mamie", "reason": "" })).await;
    let kicked = app.port("kick", json!({ "userId": "papi", "requestedBy": "mamie", "reason": "pauses" })).await;
    app.post("/sync/beat", "papi", json!({ "delay": 2.0 })).await;
    let again = app.port("kick", json!({ "userId": "papi", "requestedBy": "ced", "reason": "" })).await;

    assert_eq!(absent["Ok"]["reason"], "notInParty");
    assert_eq!(kicked["Ok"], json!({ "outcome": "kicked", "reason": null, "disruptions": 0 }));
    assert_eq!(again["Ok"]["reason"], "recentlyKicked");
    assert_eq!(app.party.lock().sync.member_count(), 1);
}

#[tokio::test]
async fn a_kick_counts_the_disruptions_the_journal_holds() {
    let app = app();
    app.watching(&["mamie", "papi"]);
    app.post("/sync/beat", "papi", json!({ "delay": 2.0 })).await;
    let now = app.party.time_now();
    app.party.lock().sync.log(now, json!({ "event": "decrochage", "userId": "papi" }));
    app.party.lock().sync.log(now, json!({ "event": "plancher_releve", "userId": "papi" }));

    let kicked = app.port("kick", json!({ "userId": "papi", "requestedBy": "mamie", "reason": "" })).await;

    assert_eq!(kicked["Ok"]["disruptions"], 2);
}

#[tokio::test]
async fn a_disconnected_member_is_not_kicked_through_the_agent_door() {
    let app = app();
    app.watching(&["mamie"]);
    app.post("/sync/beat", "papi", json!({ "delay": 2.0 })).await;

    let kicked = app.port("kick", json!({ "userId": "papi", "requestedBy": "mamie", "reason": "" })).await;

    assert_eq!(kicked["Ok"]["reason"], "disconnected");
    assert_eq!(app.party.lock().sync.member_count(), 1);
}

#[tokio::test]
async fn any_present_viewer_reads_the_journal_of_another_viewer_through_the_agent_door() {
    let app = app();
    app.watching(&["mamie", "papi"]);
    app.post("/clientlog", "papi", json!({ "events": [{ "category": "player", "event": "gel", "data": {} }] })).await;

    let other = app.port("clientLogs", json!({ "requestedBy": "mamie", "userId": "papi", "limit": 10 })).await;
    let all = app.port("clientLogs", json!({ "requestedBy": "mamie", "limit": 10 })).await;

    assert_eq!(other["Ok"]["events"][0]["event"], "gel");
    assert_eq!(all["Ok"]["events"][0]["event"], "gel");
}

#[tokio::test]
async fn a_second_poll_is_refused_while_one_is_open_and_any_present_viewer_may_close_it() {
    let app = app();
    app.watching(&["mamie", "papi"]);
    app.port("startPoll", json!({ "requestedBy": "mamie", "question": "Qui ?", "kind": "yesno" })).await;

    let second = app.port("startPoll", json!({ "requestedBy": "papi", "question": "Et toi ?", "kind": "yesno" })).await;
    let closed = app.port("closePoll", json!({ "requestedBy": "papi" })).await;

    assert_eq!(second["Err"], "pollAlreadyActive");
    assert_eq!(closed["Ok"]["results"]["question"], "Qui ?");
}

#[tokio::test]
async fn the_http_journal_stays_all_for_an_administrator_and_own_for_a_member() {
    let app = app();
    app.seen_as_admin().await;

    let (all_member, _) = app.get("/clientlog", "mamie").await;
    let (other_member, _) = app.get("/clientlog?userId=papi", "mamie").await;
    let (own_member, _) = app.get("/clientlog?userId=mamie", "mamie").await;
    let (all_admin, _) = app.get("/clientlog", "cyril").await;

    let codes = (all_member, other_member, own_member, all_admin);
    assert_eq!(codes, (StatusCode::FORBIDDEN, StatusCode::FORBIDDEN, StatusCode::OK, StatusCode::OK));
}

#[tokio::test]
async fn an_administrator_who_lost_the_role_cannot_force_from_the_next_authenticated_request() {
    let app = app();
    app.seen_as_admin().await;
    app.watching(&["mamie", "cyril"]);
    app.party.note_role("cyril", false);

    let forced = app.port("requestSwitch", json!({ "channelId": "m6", "requestedBy": "cyril", "reason": "", "force": true })).await;

    assert_eq!(forced["Ok"]["outcome"], "voteStarted");
}

#[tokio::test]
async fn force_is_honoured_for_a_seen_administrator_without_being_listed() {
    let app = app();
    app.seen_as_admin().await;
    app.watching(&["mamie", "cyril"]);
    let forced = app.port("requestSwitch", json!({ "channelId": "m6", "requestedBy": "cyril", "reason": "", "force": true })).await;
    assert_eq!(forced["Ok"]["outcome"], "switchedDirectly");
}
