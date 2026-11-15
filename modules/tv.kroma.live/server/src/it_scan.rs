use std::time::Duration;

use serde_json::json;

use crate::it_live::{load, rig, rig_with, until};
use crate::it_stream::playing;
use crate::runtime::Programs;

fn jpeg(path: &std::path::Path) -> bool {
    std::fs::read(path).is_ok_and(|bytes| bytes.starts_with(&[0xFF, 0xD8, 0xFF]))
}

#[tokio::test(flavor = "multi_thread")]
async fn a_scan_captures_real_frames_and_tells_black_still_and_moving_pictures_apart() {
    let rig = rig();
    load(&rig);

    let (id, repeated) = rig.live.start_benchmark(&["p:1".into(), "p:3".into(), "p:6".into()]).unwrap();

    assert!(!repeated);
    assert!(until(Duration::from_secs(120), || !rig.live.scan_state().running).await, "{:?}", rig.live.scan_state());
    let state = rig.live.scan_state();
    assert_eq!((state.id.as_str(), state.total, state.items.len()), (id.as_str(), 3, 3));
    let by_id = |wanted: &str| state.items.iter().find(|item| item.id == wanted).unwrap().clone();
    let (moving, still, black) = (by_id("p:1"), by_id("p:3"), by_id("p:6"));
    assert!(moving.ok && !moving.black && !moving.still && moving.frames.len() == 2, "{moving:?}");
    assert!(still.ok && still.still && still.frames.len() == 1, "{still:?}");
    assert!(black.ok && black.black, "{black:?}");
    for item in &state.items {
        let frame = rig.live.public_dir().join(item.frame.as_ref().unwrap());
        assert!(jpeg(&frame), "{frame:?}");
        assert!(!std::fs::read_dir(rig.live.public_dir().join(format!("scan-{id}"))).unwrap().any(|e| e.unwrap().file_name().to_string_lossy().starts_with("work-")), "work folders are removed");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_scan_publishes_one_event_per_item_and_a_last_one_when_done() {
    let rig = rig();
    load(&rig);

    let (id, _) = rig.live.start_benchmark(&["p:1".into(), "p:6".into()]).unwrap();

    assert!(until(Duration::from_secs(90), || !rig.live.scan_state().running && !rig.live.sent_on("live.scan").is_empty()).await);
    assert!(until(Duration::from_secs(5), || rig.live.sent_on("live.scan").len() == 3).await, "{:?}", rig.live.sent_on("live.scan"));
    let events = rig.live.sent_on("live.scan");
    assert!(events.iter().all(|e| e["id"] == json!(id)));
    assert_eq!(events[0]["item"]["id"], "p:1");
    assert_eq!(events[0]["item"]["ok"], true);
    assert_eq!(events[1]["item"]["black"], true);
    assert_eq!(events[2], json!({ "id": id, "done": true }));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_benchmark_refuses_unknown_mixed_empty_and_concurrent_requests_and_reuses_a_fresh_result() {
    let rig = rig();
    load(&rig);

    assert_eq!(rig.live.start_benchmark(&["p:999".into()]), Err("unknownChannel".to_string()));
    assert_eq!(rig.live.start_benchmark(&["p:1".into(), "s:1".into()]), Err("mixedProviders".to_string()));
    assert_eq!(rig.live.start_benchmark(&[]), Err("noChannel".to_string()));
    let (id, _) = rig.live.start_benchmark(&["p:6".into()]).unwrap();
    assert_eq!(rig.live.start_benchmark(&["p:1".into()]), Err("scanRunning".to_string()));
    assert!(until(Duration::from_secs(60), || !rig.live.scan_state().running).await);

    let (again, repeated) = rig.live.start_benchmark(&["p:6".into()]).unwrap();

    assert!(repeated && again == id);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_capture_beside_the_live_at_the_single_connection_backup_is_the_only_exception_and_can_be_switched_off() {
    let rig = rig();
    load(&rig);
    playing(&rig, "s:1").await;

    let (_, _) = rig.live.start_benchmark(&["s:3".into()]).unwrap();

    assert!(until(Duration::from_secs(60), || !rig.live.scan_state().running).await);
    assert!(rig.live.scan_state().items[0].ok, "{:?}", rig.live.scan_state().items);
    assert!(rig.backup.max_open.load(std::sync::atomic::Ordering::SeqCst) <= 2, "the live and one capture, never more");
    rig.live.shutdown();

    let strict = rig_with(Programs::default(), |h| h.with_config("backupScanWithLive", json!(false)));
    load(&strict);
    playing(&strict, "s:1").await;
    strict.live.start_benchmark(&["s:3".into()]).unwrap();
    assert!(until(Duration::from_secs(30), || !strict.live.scan_state().running).await);
    assert_eq!(strict.live.scan_state().items[0].error.as_deref(), Some("busy"));
    strict.live.shutdown();
}
