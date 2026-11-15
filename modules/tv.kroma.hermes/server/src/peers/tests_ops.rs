use std::sync::Arc;

use axum::routing::post;
use axum::{Json, Router};
use kroma_module_sdk::host::testing::StubHost;
use kroma_module_sdk::host::HostCtx;
use kroma_module_sdk::testing::{blocking, serve};
use serde_json::{json, Value};

use super::*;

type Seen = Arc<std::sync::Mutex<Vec<(String, Value)>>>;

async fn answer(point: &'static str, replies: Vec<(&'static str, Result<Value, &'static str>)>) -> (Arc<dyn HostCtx>, Seen) {
    let seen: Seen = Arc::default();
    let mut router: Router<()> = Router::new();
    for (method, reply) in replies {
        let log = seen.clone();
        router = router.route(
            &format!("/_port/{point}/{method}"),
            post(move |Json(body): Json<Value>| {
                let log = log.clone();
                let reply = reply.clone();
                async move {
                    log.lock().unwrap().push((method.to_string(), body));
                    Json(reply.map_err(str::to_string))
                }
            }),
        );
    }
    let resolver = serve(router, ()).await;
    let (base, token) = resolver().unwrap();
    (Arc::new(StubHost::new().with_point(point, None, &base, &token)), seen)
}

fn sent(seen: &Seen) -> std::collections::BTreeMap<String, Value> {
    seen.lock().unwrap().iter().cloned().collect()
}

#[tokio::test]
async fn the_live_operation_calls_send_the_pinned_json_and_read_the_pinned_replies() {
    let (host, seen) = answer(
        CONTROL,
        vec![
            ("restart", Ok(json!({ "restarted": false, "error": "streamHealthy" }))),
            ("switchProvider", Ok(json!({ "switched": false, "provider": "backup", "error": "voteRequired" }))),
            ("probe", Ok(json!({ "channelId": "p:2", "name": "CANAL+", "provider": "primary", "alive": true, "resolution": "1920x1080", "codec": "H264", "audioCodec": "AAC", "fps": 25.0, "bitrateKbps": 5120.5, "offAir": false, "local": false, "error": null }))),
            ("benchmark", Ok(json!({ "status": "started", "id": "4", "reason": null, "total": 2, "alreadyDone": false }))),
            ("scanResult", Ok(json!({ "id": "4", "query": "ligue1+", "running": false, "provider": "backup", "total": 2, "items": [{ "number": 3, "id": "s:3", "name": "LIGUE1+ 3", "ok": true, "black": false, "still": true, "frame": "scan-4/f_02.jpg", "frames": ["scan-4/f_01.jpg", "scan-4/f_02.jpg"], "error": null, "now": "Lens - Paris FC", "at": 1_794_000_123_000_i64 }] }))),
            ("history", Ok(json!({ "stepSeconds": 10, "points": [{ "at": 1_794_000_000, "kbps": 4100, "channel": "TF1", "provider": "primary", "health": "healthy", "loss": 0.5, "frozen": false, "black": false, "restarts": 2, "paused": false }], "providerEvents": [{ "kind": "failover", "at": 5 }] }))),
            ("scanFrame", Ok(json!({ "jpegBase64": "/9j/AA==" }))),
        ],
    )
    .await;
    let control = PeerControl::new(host);
    let out = blocking(move || {
        (
            control.restart("u1", "gel").unwrap(),
            control.switch_provider("backup", None, "u1", "saccades").unwrap(),
            control.probe("p:2").unwrap(),
            control.benchmark(&["s:3".into(), "s:4".into()], "u1").unwrap(),
            control.scan_result().unwrap(),
            control.history(60).unwrap(),
            control.scan_frame("scan-4/f_02.jpg").unwrap(),
        )
    })
    .await;
    assert_eq!(out.0, Restart { restarted: false, error: Some("streamHealthy".into()) });
    assert_eq!(out.1, ProviderSwitch { switched: false, provider: "backup".into(), error: Some("voteRequired".into()) });
    assert_eq!((out.2.name.as_str(), out.2.audio_codec.as_deref(), out.2.bitrate_kbps, out.2.alive), ("CANAL+", Some("AAC"), Some(5120.5), true));
    assert_eq!(out.3, BenchmarkStart { status: "started".into(), id: Some("4".into()), reason: None, total: 2, already_done: false });
    assert_eq!((out.4.items[0].at, out.4.items[0].frames.len(), out.4.items[0].still, out.4.items[0].now.as_deref(), out.4.provider.as_deref()), (1_794_000_123_000, 2, true, Some("Lens - Paris FC"), Some("backup")));
    assert_eq!((out.5.points[0].kbps, out.5.points[0].restarts, out.5.provider_events.len()), (4100.0, 2, 1));
    assert_eq!(out.6, "/9j/AA==");
    let calls = sent(&seen);
    assert_eq!(calls["restart"], json!({ "requestedBy": "u1", "reason": "gel" }));
    assert_eq!(calls["switchProvider"], json!({ "provider": "backup", "requestedBy": "u1", "reason": "saccades" }));
    assert_eq!(calls["probe"], json!({ "channelId": "p:2" }));
    assert_eq!(calls["benchmark"], json!({ "channelIds": ["s:3", "s:4"], "requestedBy": "u1" }));
    assert_eq!(calls["scanResult"], json!({}));
    assert_eq!(calls["history"], json!({ "limit": 60 }));
    assert_eq!(calls["scanFrame"], json!({ "path": "scan-4/f_02.jpg" }));
}

#[tokio::test]
async fn a_provider_switch_names_the_channel_only_when_there_is_one() {
    let (host, seen) = answer(CONTROL, vec![("switchProvider", Ok(json!({ "switched": false, "provider": "backup", "error": "providerMismatch" })))]).await;
    let control = PeerControl::new(host);

    let out = blocking(move || (control.switch_provider("backup", Some("s:12"), "u1", "même match").unwrap(), control.switch_provider("backup", None, "u1", "").unwrap())).await;

    assert_eq!(out.0.error.as_deref(), Some("providerMismatch"));
    let bodies: Vec<Value> = seen.lock().unwrap().iter().map(|(_, b)| b.clone()).collect();
    assert_eq!(bodies[0], json!({ "provider": "backup", "channelId": "s:12", "requestedBy": "u1", "reason": "même match" }));
    assert_eq!(bodies[1], json!({ "provider": "backup", "requestedBy": "u1", "reason": "" }));
}

#[tokio::test]
async fn a_scan_item_without_a_capture_time_reads_as_zero() {
    let (host, _) = answer(CONTROL, vec![("scanResult", Ok(json!({ "id": "1", "items": [{ "id": "p:1", "name": "TF1", "ok": true }] })))]).await;
    let control = PeerControl::new(host);

    let state = blocking(move || control.scan_result().unwrap()).await;

    assert_eq!(state.items[0].at, 0);
}

#[tokio::test]
async fn resolve_reads_the_provider_and_the_event_and_sends_the_provider_filter() {
    let (host, seen) = answer(CONTROL, vec![("resolve", Ok(json!({ "channels": [{ "id": "s:5", "name": "LIGUE1+ 5", "group": "FR", "score": 4.0, "now": null, "provider": "backup", "event": { "search": "ligue1+", "competition": "Ligue 1" } }] })))]).await;
    let control = PeerControl::new(host);

    let found = blocking(move || control.resolve("ligue1+ 5", 3, Some("backup")).unwrap()).await;

    assert_eq!((found[0].provider.as_str(), found[0].event.clone()), ("backup", Some(EventInfo { search: "ligue1+".into(), competition: "Ligue 1".into() })));
    assert_eq!(sent(&seen)["resolve"], json!({ "query": "ligue1+ 5", "limit": 3, "provider": "backup" }));
}

#[tokio::test]
async fn an_error_code_of_the_live_module_reaches_the_caller_as_the_message() {
    let (host, _) = answer(CONTROL, vec![("snapshot", Err("settling"))]).await;
    let control = PeerControl::new(host);

    let error = blocking(move || control.snapshot().unwrap_err()).await;

    assert_eq!(format!("{error:#}"), "settling");
    assert!(!is_unsupported(&error));
}

#[tokio::test]
async fn the_party_calls_send_the_pinned_json_and_read_the_pinned_replies() {
    let (host, seen) = answer(
        ROOM,
        vec![
            ("requestSwitch", Ok(json!({ "outcome": "voteStarted", "voteId": "9", "reason": null }))),
            ("goLive", Ok(json!({ "members": 2 }))),
            ("fineSync", Err("needTwoMembers")),
            ("clientLogs", Ok(json!({ "events": [{ "at": 1, "userId": "u1", "name": "Ana", "device": "iPhone", "category": "player", "event": "relance_gel", "data": {} }] }))),
        ],
    )
    .await;
    let room = PeerRoom::new(host);
    let out = blocking(move || {
        let plain = room.request_switch(&SwitchAsk { channel_id: "tf1", channel_name: None, requested_by: "u1", reason: "foot", force: false }).unwrap();
        let forced = room.request_switch(&SwitchAsk { channel_id: "p:3", channel_name: Some("TF1"), requested_by: "u1", reason: "", force: true }).unwrap();
        (plain, forced, room.go_live("u1").unwrap(), room.fine_sync("u1").unwrap_err(), room.client_logs("u1", Some("u1"), 50, Some(30)).unwrap(), room.client_logs("u1", None, 10, None).unwrap())
    })
    .await;
    assert_eq!(out.0.vote_id.as_deref(), Some("9"));
    assert_eq!(out.2["members"], 2);
    assert_eq!(format!("{:#}", out.3), "needTwoMembers");
    assert_eq!((out.4.len(), out.4[0]["event"].clone()), (1, json!("relance_gel")));
    let all: Vec<(String, Value)> = seen.lock().unwrap().clone();
    let switches: Vec<&Value> = all.iter().filter(|(m, _)| m == "requestSwitch").map(|(_, b)| b).collect();
    assert_eq!(switches[0], &json!({ "channelId": "tf1", "requestedBy": "u1", "reason": "foot" }));
    assert_eq!(switches[1], &json!({ "channelId": "p:3", "requestedBy": "u1", "reason": "", "channelName": "TF1", "force": true }));
    let calls = sent(&seen);
    assert_eq!(calls["goLive"], json!({ "requestedBy": "u1" }));
    assert_eq!(calls["fineSync"], json!({ "requestedBy": "u1" }));
    let logs: Vec<&Value> = all.iter().filter(|(m, _)| m == "clientLogs").map(|(_, b)| b).collect();
    assert_eq!(logs[0], &json!({ "requestedBy": "u1", "userId": "u1", "limit": 50, "minutes": 30 }));
    assert_eq!(logs[1], &json!({ "requestedBy": "u1", "userId": null, "limit": 10 }));
}

#[test]
fn a_control_that_offers_none_of_the_later_methods_answers_like_an_older_live_module() {
    struct Old;
    impl Control for Old {
        fn status(&self) -> anyhow::Result<Status> {
            Ok(Status::default())
        }
        fn resolve(&self, _: &str, _: u32, _: Option<&str>) -> anyhow::Result<Vec<Candidate>> {
            Ok(Vec::new())
        }
        fn epg(&self, _: Option<&str>, _: Option<i64>, _: Option<i64>, _: u32) -> anyhow::Result<Vec<Programme>> {
            Ok(Vec::new())
        }
        fn snapshot(&self) -> anyhow::Result<Snapshot> {
            Err(not_offered())
        }
    }

    for error in [Old.restart("u", "").unwrap_err(), Old.probe("p:1").unwrap_err(), Old.history(1).unwrap_err(), Old.scan_frame("x").unwrap_err()] {
        assert!(is_unsupported(&error));
    }
}
