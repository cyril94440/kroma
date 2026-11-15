use std::time::Duration;

use serde_json::json;

use crate::clipbuf::CLIPS_DIR;
use crate::it_live::{load, rig, until, Rig};
use crate::it_points::ask;
use crate::runtime::now_ms;

const TOKEN: &str = "0123456789abcdef0123456789abcdef";

fn plant_clip(rig: &Rig, name: &str) -> String {
    let relative = format!("{CLIPS_DIR}/{TOKEN}/{name}.mp4");
    let file = rig.live.public_dir().join(&relative);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, b"clip").unwrap();
    relative
}

async fn scanned(rig: &Rig) -> serde_json::Value {
    let started = ask(rig, "benchmark", json!({ "channelIds": ["p:1"], "requestedBy": "u1" })).await;
    assert_eq!(started["Ok"]["status"], "started");
    assert!(until(Duration::from_secs(90), || !rig.live.scan_state().running).await);
    ask(rig, "scanResult", json!({})).await["Ok"]["items"][0].clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_clip_list_names_the_cut_clips_and_the_delete_removes_one_with_its_folder() {
    let rig = rig();
    assert_eq!(ask(&rig, "clipList", json!({})).await, json!({ "Ok": { "paths": [] } }));
    let (first, second) = (plant_clip(&rig, "but-1"), plant_clip(&rig, "but-2"));

    assert_eq!(ask(&rig, "clipList", json!({})).await, json!({ "Ok": { "paths": [first, second] } }));
    assert_eq!(ask(&rig, "clipDelete", json!({ "path": first })).await, json!({ "Ok": { "deleted": true } }));
    assert_eq!(ask(&rig, "clipDelete", json!({ "path": first })).await, json!({ "Ok": { "deleted": false } }));
    assert_eq!(ask(&rig, "clipList", json!({})).await, json!({ "Ok": { "paths": [second] } }));
    assert_eq!(ask(&rig, "clipDelete", json!({ "path": second })).await, json!({ "Ok": { "deleted": true } }));
    assert!(!rig.live.public_dir().join(CLIPS_DIR).join(TOKEN).exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn clip_delete_refuses_any_path_that_is_not_a_clip_the_module_cut() {
    let rig = rig();
    let outside = rig.live.public_dir().join("secret.mp4");
    std::fs::write(&outside, b"keep").unwrap();

    for path in ["../secret.mp4", "clips/../secret.mp4", &format!("clips/{TOKEN}/../../secret.mp4"), "secret.mp4", &format!("/clips/{TOKEN}/but.mp4"), ""] {
        assert_eq!(ask(&rig, "clipDelete", json!({ "path": path })).await, json!({ "Err": "invalidPath" }), "{path}");
    }
    assert!(outside.is_file());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_scan_item_carries_its_capture_time_and_scan_frame_returns_exactly_the_frames_it_emitted() {
    let rig = rig();
    load(&rig);
    let before = now_ms();

    let item = scanned(&rig).await;

    let at = item["at"].as_i64().unwrap();
    assert!(at >= before && at <= now_ms(), "{at}");
    let frame = item["frame"].as_str().unwrap().to_string();
    let reply = ask(&rig, "scanFrame", json!({ "path": frame })).await;
    assert_eq!(reply["Ok"].as_object().unwrap().keys().collect::<Vec<_>>(), ["jpegBase64"]);
    assert!(reply["Ok"]["jpegBase64"].as_str().unwrap().starts_with("/9j/"));
    for first in item["frames"].as_array().unwrap() {
        assert!(ask(&rig, "scanFrame", json!({ "path": first })).await["Ok"].is_object());
    }
    rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn scan_frame_refuses_traversal_unknown_frames_and_files_that_are_not_scan_frames() {
    let rig = rig();
    load(&rig);
    let item = scanned(&rig).await;
    let frame = item["frame"].as_str().unwrap().to_string();
    let folder = frame.split('/').next().unwrap().to_string();
    let outside = rig.live.public_dir().join("secret.jpg");
    std::fs::write(&outside, b"\xFF\xD8\xFFkeep").unwrap();

    for path in [format!("{folder}/../secret.jpg"), "../secret.jpg".into(), "secret.jpg".into(), format!("{folder}/other.png"), String::new()] {
        assert_eq!(ask(&rig, "scanFrame", json!({ "path": path })).await, json!({ "Err": "invalidPath" }), "{path}");
    }
    assert_eq!(ask(&rig, "scanFrame", json!({ "path": format!("{folder}/999999.jpg") })).await, json!({ "Err": "notFound" }));
    std::fs::write(rig.live.public_dir().join(&folder).join("planted.jpg"), b"\xFF\xD8\xFFx").unwrap();
    assert_eq!(ask(&rig, "scanFrame", json!({ "path": format!("{folder}/planted.jpg") })).await, json!({ "Err": "notFound" }), "a file the scan did not emit is not served");
    std::fs::write(rig.live.public_dir().join(&frame), vec![0u8; 1024 * 1024 + 1]).unwrap();
    assert_eq!(ask(&rig, "scanFrame", json!({ "path": frame })).await, json!({ "Err": "tooLarge" }));
}
