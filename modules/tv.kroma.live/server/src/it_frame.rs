use std::time::Duration;

use kroma_module_sdk::primitives::input;

use crate::it_live::{load, request, rig, Rig};
use crate::it_stream::{playing, pump};
use crate::runtime::{FrameError, SETTLE_FOR_TESTS};

fn jpeg_height(bytes: &[u8]) -> u32 {
    let dir = kroma_module_sdk::testing::temp_dir("live-jpeg");
    let path = dir.path().join("shot.jpg");
    std::fs::write(&path, bytes).unwrap();
    let out = std::process::Command::new("ffprobe").args(["-v", "error", "-select_streams", "v:0", "-show_entries", "stream=height", "-of", "csv=p=0"]).args(input::file(&path)).output().unwrap();
    String::from_utf8_lossy(&out.stdout).trim().parse().unwrap_or(0)
}

async fn settled(rig: &Rig) {
    playing(rig, "p:1").await;
    tokio::time::sleep(SETTLE_FOR_TESTS + Duration::from_secs(1)).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn no_picture_is_taken_in_standby_or_while_the_source_is_down() {
    let rig = rig();
    load(&rig);
    assert_eq!(rig.live.grab_frame(false).await, Err(FrameError::Standby));
    playing(&rig, "p:1").await;
    rig.live.stop_run().await;

    assert_eq!(rig.live.grab_frame(false).await, Err(FrameError::Stopped));
    rig.live.pause().await;
    assert_eq!(rig.live.grab_frame(false).await, Err(FrameError::Standby));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_picture_waits_for_the_stream_to_settle_after_a_channel_change() {
    let rig = rig();
    load(&rig);
    playing(&rig, "p:1").await;

    let early = rig.live.grab_frame(false).await;

    assert_eq!(early, Err(FrameError::Settling));
    rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_settled_stream_gives_a_small_jpeg_at_most_720_high_taken_from_the_local_output() {
    let rig = rig();
    load(&rig);
    settled(&rig).await;

    let frame = rig.live.grab_frame(false).await.unwrap();

    assert_eq!(&frame.jpeg[..3], [0xFF, 0xD8, 0xFF]);
    assert!(frame.jpeg.len() < 300_000, "{}", frame.jpeg.len());
    let height = jpeg_height(&frame.jpeg);
    assert!(height > 0 && height <= 720, "{height}");
    assert_eq!(rig.primary.max_open.load(std::sync::atomic::Ordering::SeqCst), 1, "no second connection to the provider");
    rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_second_request_within_fifteen_seconds_is_served_from_the_first_picture() {
    let rig = rig();
    load(&rig);
    settled(&rig).await;
    let first = rig.live.grab_frame(false).await.unwrap();
    tokio::time::sleep(Duration::from_millis(1200)).await;

    let again = rig.live.grab_frame(false).await.unwrap();
    let fresh = rig.live.grab_frame(true).await.unwrap();

    assert_eq!(again.at, first.at);
    assert!(fresh.at > first.at);
    rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_zap_makes_the_next_request_wait_again() {
    let rig = rig();
    load(&rig);
    settled(&rig).await;
    rig.live.grab_frame(false).await.unwrap();

    playing(&rig, "p:3").await;

    assert_eq!(rig.live.grab_frame(false).await, Err(FrameError::Settling));
    rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn the_source_resolution_is_read_from_the_local_output_and_cleared_by_a_zap() {
    let rig = rig();
    load(&rig);
    playing(&rig, "p:1").await;

    assert!(pump(&rig, Duration::from_secs(30), |r| !r.live.status().source_resolution.is_empty()).await, "no resolution");

    assert_eq!(rig.live.status().source_resolution, "320x180@25");
    rig.live.switch(request("p:3")).await;
    assert_eq!(rig.live.status().source_resolution, "");
    rig.live.shutdown();
}
