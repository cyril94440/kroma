use axum::body::Body;
use axum::http::Request;
use kroma_module_sdk::host::testing::StubHost;
use serde_json::{json, Value};
use tower::ServiceExt;

use crate::it_live::{load, rig, Rig};

pub(crate) async fn ask(rig: &Rig, method: &str, body: Value) -> Value {
    let host = rig.host.clone().with_service(rig.live.clone());
    let app = crate::routes::point_routes::<StubHost>().with_state(host);
    let request = Request::builder().method("POST").uri(format!("/_port/tv.kroma.live/control/{method}")).header("content-type", "application/json").body(Body::from(body.to_string())).unwrap();
    let response = app.oneshot(request).await.unwrap();
    serde_json::from_slice(&axum::body::to_bytes(response.into_body(), 1 << 22).await.unwrap()).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_status_point_answers_exactly_the_fixed_contract() {
    let rig = rig();

    let reply = ask(&rig, "status", json!({})).await;

    assert_eq!(
        reply,
        json!({ "Ok": {
            "state": "paused", "channel": null, "provider": "primary", "now": null, "next": null, "viewers": [],
            "health": { "sourceKbps": 0.0, "stutter": false, "black": false, "frozen": false },
            "startedAt": null
        } })
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_status_point_names_who_watches_and_what_is_on() {
    let rig = rig();
    load(&rig);
    rig.live.lock().presence.beat("u1", "Ann", crate::runtime::now_ms());
    let outcome = rig.live.switch(crate::it_live::request("p:1")).await;
    assert_eq!(outcome.status, "switched");

    let reply = ask(&rig, "status", json!({})).await;

    let ok = &reply["Ok"];
    assert_eq!(ok["state"], "running");
    assert_eq!(ok["channel"], json!({ "id": "p:1", "name": "FR: TF1 FHD" }));
    assert_eq!(ok["now"]["title"], "Journal");
    assert_eq!(ok["next"]["title"], "Film du soir");
    assert!(ok["now"]["start"].as_i64().unwrap() % 1000 == 0 && ok["now"]["end"].as_i64() > ok["now"]["start"].as_i64());
    assert_eq!(ok["viewers"], json!([{ "userId": "u1", "name": "Ann" }]));
    let started = ok["startedAt"].as_i64().unwrap();
    assert!(started > 1_700_000_000_000 && started <= crate::runtime::now_ms());
    assert_eq!(started, rig.live.status().started_at.unwrap());
    rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn the_resolve_point_answers_scored_channels_with_what_is_on_now() {
    let rig = rig();
    load(&rig);

    let reply = ask(&rig, "resolve", json!({ "query": "tf1", "limit": 3 })).await;

    let channel = &reply["Ok"]["channels"][0];
    assert_eq!((channel["id"].as_str(), channel["name"].as_str(), channel["group"].as_str()), (Some("p:1"), Some("FR: TF1 FHD"), Some("General")));
    assert!(channel["score"].is_number());
    assert_eq!(channel["now"]["title"], "Journal");
    assert_eq!(ask(&rig, "resolve", json!({ "query": "zzzz" })).await, json!({ "Ok": { "channels": [] } }));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_switch_point_switches_by_id_or_words_and_says_why_when_it_cannot() {
    let rig = rig();
    load(&rig);

    let by_id = ask(&rig, "switch", json!({ "channelId": "p:3", "requestedBy": "u1", "reason": "vote passed" })).await;
    let by_words = ask(&rig, "switch", json!({ "channelId": "canal plus", "requestedBy": "u1", "reason": "vote" })).await;
    let missing = ask(&rig, "switch", json!({ "channelId": "p:999", "requestedBy": "u1", "reason": "vote" })).await;

    assert_eq!(by_id, json!({ "Ok": { "switched": true, "channel": { "id": "p:3", "name": "FR: M6 HD" }, "error": null } }));
    assert_eq!(by_words["Ok"]["channel"]["name"], "VIP: CANAL+ ᴿᴬᵂ");
    assert_eq!(missing, json!({ "Ok": { "switched": false, "channel": null, "error": "unknown channel" } }));
    rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn the_presence_point_lists_viewers_with_when_they_arrived() {
    let rig = rig();
    rig.live.lock().presence.beat("u1", "Ann", 1_234);

    let reply = ask(&rig, "presence", json!({})).await;

    assert_eq!(reply, json!({ "Ok": { "viewers": [{ "userId": "u1", "name": "Ann", "since": 1234 }] } }));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_proposal_point_applies_nothing_for_an_unknown_or_declined_proposal() {
    let rig = rig();

    assert_eq!(ask(&rig, "proposal", json!({ "id": "nope", "accept": true })).await, json!({ "Ok": { "applied": false } }));
    rig.live.lock().proposals.insert("p1".into(), crate::status::Proposal { id: "p1".into(), channel: Some("p:3".into()), ..Default::default() });
    assert_eq!(ask(&rig, "proposal", json!({ "id": "p1", "accept": false })).await, json!({ "Ok": { "applied": false } }));
    assert!(rig.live.lock().proposals.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn an_accepted_proposal_switches_to_its_channel() {
    let rig = rig();
    load(&rig);
    rig.live.lock().proposals.insert("p2".into(), crate::status::Proposal { id: "p2".into(), kind: "switchToBackup".into(), channel: Some("s:1".into()), ..Default::default() });

    let reply = ask(&rig, "proposal", json!({ "id": "p2", "accept": true })).await;

    assert_eq!(reply, json!({ "Ok": { "applied": true } }));
    assert!(rig.live.status().on_backup);
    rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn the_epg_point_searches_by_title_or_lists_a_window_with_millisecond_times() {
    let rig = rig();
    load(&rig);

    let by_title = ask(&rig, "epg", json!({ "query": "liverpool", "from": null, "to": null, "limit": 5 })).await;
    let window = ask(&rig, "epg", json!({ "query": null, "from": null, "to": null, "limit": 10 })).await;

    let first = &by_title["Ok"]["programmes"][0];
    assert_eq!((first["channelId"].as_str(), first["channelName"].as_str(), first["title"].as_str()), (Some("p:2"), Some("VIP: CANAL+ ᴿᴬᵂ"), Some("PSG Liverpool")));
    assert!(first["start"].as_i64().unwrap() > 1_700_000_000_000);
    assert!(window["Ok"]["programmes"].as_array().unwrap().len() >= 3);
}
