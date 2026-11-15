use std::time::Duration;

use serde_json::{json, Value};

use crate::it_live::{load, rig, rig_with, Rig};
use crate::it_points::ask;
use crate::it_stream::{playing, pump};
use crate::runtime::{now_ms, SETTLE_FOR_TESTS};

async fn healthy(rig: &Rig) {
    playing(rig, "p:1").await;
    assert!(pump(rig, Duration::from_secs(20), |r| r.live.status().health.source_kbps > 120.0).await, "{:?}", rig.live.status().health);
}

fn viewers(rig: &Rig, ids: &[&str]) {
    for id in ids {
        rig.live.lock().presence.beat(id, id, now_ms());
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_snapshot_point_answers_a_standby_refusal_then_the_exact_jpeg_shape() {
    let rig = rig();
    load(&rig);
    assert_eq!(ask(&rig, "snapshot", json!({})).await, json!({ "Err": "standby" }));
    playing(&rig, "p:1").await;
    tokio::time::sleep(SETTLE_FOR_TESTS + Duration::from_secs(1)).await;

    let reply = ask(&rig, "snapshot", json!({})).await;

    let ok = reply["Ok"].as_object().unwrap();
    assert_eq!(ok.keys().cloned().collect::<Vec<_>>(), ["at", "jpegBase64"]);
    assert!(ok["jpegBase64"].as_str().unwrap().starts_with("/9j/"));
    assert!(ok["jpegBase64"].as_str().unwrap().len() < 400_000);
    assert!(ok["at"].as_i64().unwrap() > 1_700_000_000_000);
    rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn the_clip_points_report_the_buffer_and_cut_into_the_published_folder() {
    let rig = rig();
    load(&rig);
    assert_eq!(ask(&rig, "clipBufferInfo", json!({})).await, json!({ "Ok": { "fromMs": null, "toMs": null } }));
    playing(&rig, "p:1").await;
    for _ in 0..40 {
        rig.live.tick_blocking_buffer();
        if rig.live.clip_buffer_info().is_some_and(|(a, b)| b - a >= 8000) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    let info = ask(&rig, "clipBufferInfo", json!({})).await;
    let (from, to) = (info["Ok"]["fromMs"].as_i64().unwrap(), info["Ok"]["toMs"].as_i64().unwrap());

    let refused = ask(&rig, "clipCut", json!({ "fromMs": from, "toMs": to, "name": "Bad Name" })).await;
    let cut = ask(&rig, "clipCut", json!({ "fromMs": to - 7000, "toMs": to - 1000, "name": "but-1" })).await;

    assert_eq!(refused, json!({ "Err": "invalidName" }));
    let ok = cut["Ok"].as_object().unwrap();
    assert_eq!(ok.keys().cloned().collect::<Vec<_>>(), ["durationMs", "path"]);
    assert!(ok["path"].as_str().unwrap().starts_with("clips/") && ok["path"].as_str().unwrap().ends_with("/but-1.mp4"));
    assert!(ok["durationMs"].as_i64().unwrap() >= 5000);
    assert!(rig.live.public_dir().join(ok["path"].as_str().unwrap()).is_file());
    rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn restart_is_refused_for_a_healthy_stream_watched_by_others_then_allowed_alone_once_per_five_minutes() {
    let rig = rig();
    load(&rig);
    assert_eq!(ask(&rig, "restart", json!({ "requestedBy": "u1", "reason": "x" })).await, json!({ "Ok": { "restarted": false, "error": "noChannel" } }));
    healthy(&rig).await;
    viewers(&rig, &["u1", "u2"]);

    let shared = ask(&rig, "restart", json!({ "requestedBy": "u1", "reason": "it freezes" })).await;
    rig.live.lock().presence.leave("u2", now_ms());
    let alone = ask(&rig, "restart", json!({ "requestedBy": "u1", "reason": "it freezes" })).await;
    let again = ask(&rig, "restart", json!({ "requestedBy": "u1", "reason": "it freezes" })).await;

    assert_eq!(shared, json!({ "Ok": { "restarted": false, "error": "streamHealthy" } }));
    assert_eq!(alone, json!({ "Ok": { "restarted": true, "error": null } }));
    assert_eq!(again, json!({ "Ok": { "restarted": false, "error": "recentRestart" } }));
    rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn restart_is_refused_in_standby_and_allowed_with_the_servers_own_evidence() {
    let rig = rig();
    load(&rig);
    healthy(&rig).await;
    rig.live.pause().await;
    assert_eq!(ask(&rig, "restart", json!({ "requestedBy": "u1", "reason": "x" })).await, json!({ "Ok": { "restarted": false, "error": "standby" } }));
    rig.live.resume().await;
    viewers(&rig, &["u1", "u2"]);
    rig.live.lock().black = true;

    let reply = ask(&rig, "restart", json!({ "requestedBy": "u1", "reason": "black screen" })).await;

    assert_eq!(reply, json!({ "Ok": { "restarted": true, "error": null } }));
    rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn switch_provider_refuses_by_reason_asks_for_a_vote_with_others_and_switches_alone() {
    let rig = rig();
    load(&rig);
    let call = |provider: &str| json!({ "provider": provider, "requestedBy": "u1", "reason": "it stutters" });
    assert_eq!(ask(&rig, "switchProvider", call("backup")).await, json!({ "Ok": { "switched": false, "provider": "backup", "error": "noChannel" } }));
    assert_eq!(ask(&rig, "switchProvider", call("moon")).await, json!({ "Ok": { "switched": false, "provider": "moon", "error": "unknownProvider" } }));
    healthy(&rig).await;
    assert_eq!(ask(&rig, "switchProvider", call("primary")).await["Ok"]["error"], "alreadyOn");
    viewers(&rig, &["u1", "u2"]);

    let vote = ask(&rig, "switchProvider", call("backup")).await;
    rig.live.lock().presence.leave("u2", now_ms());
    let alone = ask(&rig, "switchProvider", call("backup")).await;

    assert_eq!(vote, json!({ "Ok": { "switched": false, "provider": "backup", "error": "voteRequired" } }));
    assert_eq!(rig.live.sent_on("live.proposal")[0]["proposal"]["kind"], "switchProvider");
    assert_eq!(alone, json!({ "Ok": { "switched": true, "provider": "backup", "error": null } }));
    assert!(rig.live.status().on_backup);
    assert_eq!(ask(&rig, "switchProvider", call("primary")).await["Ok"]["switched"], true, "back, alone");
    rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn switch_provider_says_when_there_is_no_backup_or_the_main_provider_is_down() {
    let rig = rig_with(Default::default(), |h| h.with_config("backupUrl", json!("")));
    load(&rig);
    playing(&rig, "p:1").await;
    assert_eq!(ask(&rig, "switchProvider", json!({ "provider": "backup", "requestedBy": "u1", "reason": "" })).await["Ok"]["error"], "noBackup");
    rig.live.shutdown();
    let down = crate::it_live::rig();
    load(&down);
    playing(&down, "s:1").await;
    down.live.lock().policy.mark_primary_down(1.0);

    let reply = ask(&down, "switchProvider", json!({ "provider": "primary", "requestedBy": "u1", "reason": "" })).await;

    assert_eq!(reply["Ok"]["error"], "primaryDown");
    down.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn the_probe_point_reports_a_channel_with_its_picture_codec_and_measured_rate() {
    let rig = rig();
    load(&rig);
    healthy(&rig).await;

    let other = ask(&rig, "probe", json!({ "channelId": "p:2" })).await;
    let local = ask(&rig, "probe", json!({ "channelId": "p:1" })).await;
    let unknown = ask(&rig, "probe", json!({ "channelId": "p:999" })).await;

    let ok = &other["Ok"];
    assert_eq!((ok["channelId"].clone(), ok["name"].clone(), ok["provider"].clone(), ok["alive"].clone(), ok["resolution"].clone(), ok["codec"].clone(), ok["audioCodec"].clone()), (json!("p:2"), json!("VIP: CANAL+ ᴿᴬᵂ"), json!("primary"), json!(true), json!("320x180"), json!("H264"), json!("AAC")));
    assert_eq!((ok["offAir"].clone(), ok["local"].clone(), ok["error"].clone()), (json!(false), json!(false), Value::Null));
    assert!(ok["bitrateKbps"].as_f64().unwrap() > 10.0, "{ok}");
    assert_eq!((local["Ok"]["local"].clone(), local["Ok"]["alive"].clone()), (json!(true), json!(true)));
    assert!(local["Ok"]["bitrateKbps"].as_f64().unwrap() > 10.0);
    assert_eq!(unknown["Ok"]["error"], "unknownChannel");
    rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn the_probe_point_refuses_to_open_a_second_connection_at_the_backup_beside_the_live() {
    let rig = rig();
    load(&rig);
    playing(&rig, "s:1").await;

    let reply = ask(&rig, "probe", json!({ "channelId": "s:3" })).await;

    assert_eq!((reply["Ok"]["alive"].clone(), reply["Ok"]["error"].clone()), (json!(false), json!("busy")));
    assert_eq!(rig.backup.max_open.load(std::sync::atomic::Ordering::SeqCst), 1);
    rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn the_benchmark_points_start_a_scan_refuse_with_a_reason_and_return_the_last_results() {
    let rig = rig();
    load(&rig);

    let refused = ask(&rig, "benchmark", json!({ "channelIds": ["p:1", "s:1"], "requestedBy": "u1" })).await;
    let started = ask(&rig, "benchmark", json!({ "channelIds": ["p:6"], "requestedBy": "u1" })).await;
    let busy = ask(&rig, "benchmark", json!({ "channelIds": ["p:1"], "requestedBy": "u1" })).await;

    assert_eq!(refused, json!({ "Ok": { "status": "refused", "id": null, "reason": "mixedProviders", "total": 2, "alreadyDone": false } }));
    let id = started["Ok"]["id"].as_str().unwrap().to_string();
    assert_eq!(started, json!({ "Ok": { "status": "started", "id": id, "reason": null, "total": 1, "alreadyDone": false } }));
    assert_eq!(busy["Ok"]["reason"], "scanRunning");
    for _ in 0..60 {
        if ask(&rig, "scanResult", json!({})).await["Ok"]["running"] == false {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    let result = ask(&rig, "scanResult", json!({})).await;
    assert_eq!((result["Ok"]["id"].clone(), result["Ok"]["running"].clone(), result["Ok"]["total"].clone(), result["Ok"]["provider"].clone()), (json!(id), json!(false), json!(1), json!("primary")));
    assert_eq!(result["Ok"]["items"][0]["id"], "p:6");
    assert_eq!(result["Ok"]["items"][0]["black"], true);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_history_point_returns_the_latest_points_and_the_provider_events() {
    let rig = rig();
    load(&rig);
    playing(&rig, "p:1").await;
    for _ in 0..3 {
        rig.live.tick().await;
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    crate::db::add_provider_event(&rig.pool, &crate::db::ProviderEvent { at: 5, kind: "failover".into(), channel: Some("TF1".into()), ..Default::default() }).unwrap();

    let reply = ask(&rig, "history", json!({ "limit": 5 })).await;

    let ok = &reply["Ok"];
    assert_eq!(ok["stepSeconds"], 10);
    assert!(!ok["points"].as_array().unwrap().is_empty() && ok["points"].as_array().unwrap().len() <= 5);
    assert_eq!(ok["points"][0]["channel"], "FR: TF1 FHD");
    assert_eq!(ok["providerEvents"][0]["kind"], "failover");
    rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn resolve_also_labels_a_reference_and_filters_by_provider() {
    let rig = rig();
    load(&rig);

    let by_ref = ask(&rig, "resolve", json!({ "query": "p:3" })).await;
    let backup = ask(&rig, "resolve", json!({ "query": "tf1", "provider": "backup" })).await;
    let wrong = ask(&rig, "resolve", json!({ "query": "p:3", "provider": "backup" })).await;
    let plain = ask(&rig, "resolve", json!({ "query": "tf1", "provider": null })).await;

    assert_eq!((by_ref["Ok"]["channels"][0]["id"].clone(), by_ref["Ok"]["channels"][0]["name"].clone(), by_ref["Ok"]["channels"][0]["provider"].clone()), (json!("p:3"), json!("FR: M6 HD"), json!("primary")));
    assert_eq!((backup["Ok"]["channels"][0]["id"].clone(), backup["Ok"]["channels"][0]["provider"].clone()), (json!("s:1"), json!("backup")));
    assert_eq!(wrong, json!({ "Ok": { "channels": [] } }));
    assert_eq!(plain["Ok"]["channels"][0]["id"], "p:1");
}
