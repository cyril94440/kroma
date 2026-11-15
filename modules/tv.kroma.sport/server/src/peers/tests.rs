use std::sync::{Arc, Mutex};

use axum::routing::post;
use axum::{Json, Router};
use kroma_module_sdk::host::testing::StubHost;
use kroma_module_sdk::host::{HostCtx, Resolver};
use kroma_module_sdk::testing::{blocking, serve};
use serde_json::{json, Value};

use super::*;

type Seen = Arc<Mutex<Vec<(String, Value)>>>;

fn host_for(point: &str, resolver: &Resolver) -> Arc<dyn HostCtx> {
    let (base, token) = resolver().unwrap();
    Arc::new(StubHost::new().with_point(point, None, &base, &token))
}

async fn answer(
    point: &'static str,
    replies: Vec<(&'static str, Value)>,
) -> (Arc<dyn HostCtx>, Seen) {
    let seen: Seen = Arc::default();
    let mut router: Router<()> = Router::new();
    for (method, reply) in replies {
        let log = seen.clone();
        router = router.route(
            &format!("/_port/{point}/{method}"),
            post(move |Json(body): Json<Value>| {
                let (log, reply) = (log.clone(), reply.clone());
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

fn sent(seen: &Seen) -> std::collections::BTreeMap<String, Value> {
    seen.lock().unwrap().iter().cloned().collect()
}

#[tokio::test]
async fn the_control_calls_send_the_pinned_json_and_read_the_pinned_replies() {
    let (host, seen) = answer(
        CONTROL,
        vec![
            ("status", json!({"state": "running", "channel": {"id": "primary:7", "name": "Canal+ Foot"}, "provider": "primary", "now": {"title": "Match", "start": 1, "end": 2}, "viewers": [{"userId": "u1", "name": "Ana"}], "startedAt": 99, "health": {"stutter": true}})),
            ("epg", json!({"programmes": [{"channelId": "m6", "channelName": "M6", "title": "Foot", "description": "d", "start": 1, "end": 2, "extra": true}]})),
            ("resolve", json!({"channels": [{"id": "m6", "name": "M6", "group": "FR", "score": 9.5, "now": null}]})),
            ("clipBufferInfo", json!({"fromMs": 10, "toMs": 20})),
            ("clipCut", json!({"path": "clips/a.mp4", "durationMs": 110_000})),
            ("clipDelete", json!({"deleted": true})),
            ("clipList", json!({"paths": ["clips/a.mp4"]})),
            ("scanResult", json!({"id": "s1", "running": false, "total": 2, "items": [{"number": 1, "id": "p:1", "name": "FR: LIGUE1+ 1", "ok": true, "black": false, "still": false, "now": "Ligue 1 : Lens - Lille", "frames": []}, {"id": "p:2", "name": "FR: LIGUE1+ 2", "ok": true, "black": true}]})),
            ("benchmark", json!({"status": "started", "id": "b1", "reason": null, "total": 1, "alreadyDone": false})),
        ],
    )
    .await;
    let live = PeerLive::new(host);

    let out = blocking(move || {
        (
            live.status().unwrap(),
            live.epg("lens lille", 1, 2, 50).unwrap(),
            live.resolve("m6", 3).unwrap(),
            live.clip_buffer_info().unwrap(),
            live.clip_cut(100, 200, "a").unwrap(),
            live.clip_delete("clips/a.mp4").unwrap(),
            live.clip_list().unwrap(),
            live.scan_result().unwrap(),
            live.benchmark(&["p:1".to_string()], "u1").unwrap(),
        )
    })
    .await;

    assert_eq!(
        (
            out.0.channel.unwrap().id.as_str(),
            out.0.started_at,
            out.0.viewers[0].user_id.as_str()
        ),
        ("primary:7", Some(99), "u1")
    );
    assert_eq!(
        (out.1[0].description.as_str(), out.2[0].id.as_str()),
        ("d", "m6")
    );
    assert_eq!(
        (out.3, out.4, out.5, out.6),
        (
            Some(BufferInfo {
                from_ms: Some(10),
                to_ms: Some(20)
            }),
            Some(CutClip {
                path: "clips/a.mp4".into(),
                duration_ms: 110_000
            }),
            true,
            Some(vec!["clips/a.mp4".into()])
        )
    );
    let scan = out.7.unwrap();
    assert_eq!((scan.running, scan.total), (false, 2));
    assert_eq!(
        (
            scan.rows[0].channel_id.as_str(),
            scan.rows[0].state.as_str(),
            scan.rows[0].reading.as_ref().unwrap().team1.as_str()
        ),
        ("p:1", "ok", "Lens")
    );
    assert_eq!(
        (scan.rows[1].state.as_str(), scan.rows[1].reading.is_none()),
        ("noir", true)
    );
    assert_eq!(out.8.unwrap().status, "started");
    let calls = sent(&seen);
    assert_eq!(calls["status"], json!({}));
    assert_eq!(
        calls["epg"],
        json!({"query": "lens lille", "from": 1, "to": 2, "limit": 50})
    );
    assert_eq!(calls["resolve"], json!({"query": "m6", "limit": 3}));
    assert_eq!(calls["clipBufferInfo"], json!({}));
    assert_eq!(
        calls["clipCut"],
        json!({"fromMs": 100, "toMs": 200, "name": "a"})
    );
    assert_eq!(calls["clipDelete"], json!({"path": "clips/a.mp4"}));
    assert_eq!(
        calls["benchmark"],
        json!({"channelIds": ["p:1"], "requestedBy": "u1"})
    );
}

#[tokio::test]
async fn the_room_say_sends_a_system_message_with_its_card() {
    let (host, seen) = answer(
        ROOM,
        vec![
            ("say", json!({"id": "m1"})),
            (
                "requestSwitch",
                json!({"outcome": "voteStarted", "voteId": "3", "reason": null, "extra": 1}),
            ),
        ],
    )
    .await;
    let room = PeerRoom::new(host);

    let out = blocking(move || {
        (
            room.say(
                "But !",
                Some(json!({"kind": "match", "home": "A", "away": "B"})),
            )
            .unwrap(),
            room.request_switch("tf1", "u1", "foot").unwrap(),
        )
    })
    .await;

    assert_eq!(out.0, "m1");
    assert_eq!(
        out.1,
        SwitchOutcome {
            outcome: "voteStarted".into(),
            vote_id: Some("3".into()),
            reason: None
        }
    );
    let calls = sent(&seen);
    assert_eq!(
        calls["say"],
        json!({"text": "But !", "as": "system", "card": {"kind": "match", "home": "A", "away": "B"}, "replyTo": null})
    );
    assert_eq!(
        calls["requestSwitch"],
        json!({"channelId": "tf1", "requestedBy": "u1", "reason": "foot"})
    );
}

#[test]
fn a_missing_method_is_unsupported_and_a_missing_peer_is_an_error() {
    let host: Arc<dyn HostCtx> = Arc::new(StubHost::new());

    assert!(is_unsupported(&anyhow::anyhow!("HTTP 404: not found")));
    assert!(is_unsupported(&anyhow::anyhow!("HTTP 405: no")));
    assert!(!is_unsupported(&anyhow::anyhow!("HTTP 500: boom")));
    assert!(PeerLive::new(host.clone()).status().is_err());
    assert!(PeerRoom::new(host).say("x", None).is_err());
}
