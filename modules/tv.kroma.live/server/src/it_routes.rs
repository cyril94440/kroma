use axum::body::Body;
use axum::http::{Request, StatusCode};
use kroma_module_sdk::domain::Permission;
use kroma_module_sdk::host::testing::StubHost;
use serde_json::{json, Value};
use tower::ServiceExt;

use crate::it_live::{load, request, rig, Rig};

fn user(id: &str, name: &str, admin: bool) -> kroma_module_sdk::domain::User {
    kroma_module_sdk::domain::User {
        id: id.into(),
        email: format!("{id}@test.dev"),
        username: name.into(),
        avatar_url: None,
        language: None,
        audio_language: None,
        subtitle_language: None,
        permissions: if admin { vec![Permission::Playback, Permission::SettingsManage] } else { vec![Permission::Playback] },
        libraries: kroma_module_sdk::domain::LibraryScope::All,
        created_at: "2024-01-01T00:00:00Z".into(),
        has_pin: false,
    }
}

struct Api {
    app: axum::Router,
    rig: Rig,
}

fn api(party: bool) -> Api {
    let rig = rig();
    let mut host = rig.host.clone().with_session("ann", user("u1", "Ann", false)).with_session("bob", user("u2", "Bob", false)).with_session("root", user("u9", "Root", true));
    host = host.with_service(rig.live.clone());
    if party {
        host = host.with_point("tv.kroma.party/room", None, "http://127.0.0.1:1", "t");
    }
    let rig = Rig { host: host.clone(), ..rig };
    let app = crate::routes::routes::<StubHost>().merge(crate::routes::point_routes::<StubHost>()).with_state(host);
    Api { app, rig }
}

async fn call(api: &Api, method: &str, path: &str, token: Option<&str>, body: Option<Value>) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(path);
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let request = match body {
        Some(body) => builder.header("content-type", "application/json").body(Body::from(body.to_string())).unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };
    let response = api.app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 22).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

#[tokio::test(flavor = "multi_thread")]
async fn every_viewer_route_needs_a_session() {
    let api = api(false);

    for (method, path) in [("GET", "/status"), ("GET", "/channels"), ("GET", "/favourites"), ("POST", "/presence"), ("POST", "/switch"), ("GET", "/epg/search?q=a"), ("GET", "/epg/grid")] {
        let (status, _) = call(&api, method, path, None, Some(json!({}))).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {path}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_status_has_a_stable_shape_when_nothing_has_ever_played() {
    let api = api(false);

    let (status, body) = call(&api, "GET", "/status", Some("ann"), None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({
            "state": "paused", "channel": null, "provider": "primary", "onBackup": false, "now": null, "next": null, "viewers": [],
            "health": { "sourceKbps": 0.0, "stutter": false, "black": false, "frozen": false, "trouble": false, "state": "starting", "lossSeconds": 0.0 },
            "playlist": null, "startedAt": null, "restarts": 0, "standbyInSeconds": null, "lastError": null, "offAir": null,
            "backupConfigured": true, "primaryDown": false
        })
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_heartbeat_makes_a_viewer_present_and_leaving_removes_them_with_an_event_each_time() {
    let api = api(false);

    let (_, beat) = call(&api, "POST", "/presence", Some("ann"), Some(json!({}))).await;
    call(&api, "POST", "/presence", Some("ann"), Some(json!({}))).await;
    call(&api, "POST", "/presence", Some("bob"), Some(json!({ "companion": true }))).await;
    let (_, status) = call(&api, "GET", "/status", Some("bob"), None).await;

    assert_eq!(beat, json!({ "viewers": 1, "heartbeatSeconds": 15 }));
    assert_eq!(status["viewers"].as_array().unwrap().iter().map(|v| v["name"].as_str().unwrap()).collect::<Vec<_>>(), ["Ann"]);
    let (left, _) = call(&api, "POST", "/presence/leave", Some("ann"), None).await;
    assert_eq!(left, StatusCode::NO_CONTENT);
    assert_eq!(api.rig.live.viewer_count(), 0);
    let topics = api.rig.host.topics();
    assert_eq!(topics.iter().filter(|t| *t == "live.presence").count(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn channels_come_paged_with_their_groups_and_what_is_on_now() {
    let api = api(false);
    load(&api.rig);

    let (_, page) = call(&api, "GET", "/channels?perPage=2&page=1", Some("ann"), None).await;
    let (_, found) = call(&api, "GET", "/channels?q=tf1", Some("ann"), None).await;
    let (_, favourites) = call(&api, "GET", "/favourites", Some("ann"), None).await;
    let (_, search) = call(&api, "GET", "/epg/search?q=liverpool", Some("ann"), None).await;

    assert_eq!((page["total"].as_u64(), page["channels"].as_array().map(Vec::len), page["perPage"].as_u64()), (Some(4), Some(2), Some(2)));
    assert!(page["groups"].as_array().unwrap().contains(&json!("General")));
    assert_eq!(found["channels"][0]["name"], "FR: TF1 FHD");
    assert_eq!(found["channels"][0]["now"]["title"], "Journal");
    assert_eq!(favourites.as_array().unwrap().len(), 3);
    assert_eq!(search[0]["title"], "PSG Liverpool");
    assert_eq!(search[0]["channelId"], "p:2");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_viewer_switches_alone_but_must_use_the_vote_when_the_party_module_is_there_and_others_watch() {
    let api = api(true);
    load(&api.rig);
    call(&api, "POST", "/presence", Some("ann"), Some(json!({}))).await;
    call(&api, "POST", "/presence", Some("bob"), Some(json!({}))).await;

    let (refused, body) = call(&api, "POST", "/switch", Some("ann"), Some(json!({ "channelId": "p:3" }))).await;
    let (admin, outcome) = call(&api, "POST", "/switch", Some("root"), Some(json!({ "channelId": "p:3" }))).await;

    assert_eq!(refused, StatusCode::FORBIDDEN);
    assert_eq!(body["error"], "useVote");
    assert_eq!(admin, StatusCode::OK);
    assert_eq!(outcome["status"], "switched");
    api.rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn without_the_party_module_a_viewer_switches_directly_by_id_or_by_words() {
    let api = api(false);
    load(&api.rig);

    let (by_id, one) = call(&api, "POST", "/switch", Some("ann"), Some(json!({ "channelId": "p:3" }))).await;
    let (by_words, two) = call(&api, "POST", "/switch", Some("ann"), Some(json!({ "query": "canal plus" }))).await;
    let (unknown, _) = call(&api, "POST", "/switch", Some("ann"), Some(json!({ "query": "zzzzzz" }))).await;

    assert_eq!((by_id, by_words, unknown), (StatusCode::OK, StatusCode::OK, StatusCode::NOT_FOUND));
    assert_eq!(one["channel"], json!({ "id": "p:3", "name": "FR: M6 HD" }));
    assert_eq!(two["channel"]["name"], "VIP: CANAL+ ᴿᴬᵂ");
    api.rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn the_admin_routes_answer_history_metrics_overview_and_scan_state() {
    let api = api(false);
    load(&api.rig);

    let (_, overview) = call(&api, "GET", "/overview", Some("root"), None).await;
    let (_, history) = call(&api, "GET", "/history?minutes=5", Some("root"), None).await;
    let (_, metrics) = call(&api, "GET", "/metrics", Some("root"), None).await;
    let (_, scan) = call(&api, "GET", "/scan", Some("root"), None).await;
    let (paused, _) = call(&api, "POST", "/pause", Some("root"), None).await;

    assert_eq!(overview["providers"][0]["channels"], 5);
    assert_eq!(overview["providers"][1]["singleConnection"], true);
    assert!(overview["guideProgrammes"].as_i64().unwrap() >= 4);
    assert_eq!(history["stepSeconds"], 10);
    assert_eq!(metrics["zaps"], 0);
    assert_eq!(scan["running"], false);
    assert_eq!(paused, StatusCode::OK);
    let _ = request("p:1");
}
