use std::time::Duration;

use crate::clipbuf::CLIPS_DIR;
use crate::it_live::{load, request, rig, until, Rig};
use crate::runtime::ClipError;

fn ffprobe_duration(path: &std::path::Path) -> f64 {
    let out = std::process::Command::new("ffprobe").args(["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0"]).arg(path).output().unwrap();
    String::from_utf8_lossy(&out.stdout).trim().parse().unwrap_or(0.0)
}

async fn buffered(rig: &Rig, seconds: i64) -> bool {
    until(Duration::from_secs(40), || {
        rig.live.tick_blocking_buffer();
        rig.live.clip_buffer_info().is_some_and(|(from, to)| to - from >= seconds * 1000)
    })
    .await
}

#[tokio::test(flavor = "multi_thread")]
async fn the_buffer_keeps_the_segments_the_playlist_has_already_dropped() {
    let rig = rig();
    load(&rig);
    rig.live.switch(request("p:1")).await;

    assert!(buffered(&rig, 26).await, "{:?}", rig.live.clip_buffer_info());

    let (from, to) = rig.live.clip_buffer_info().unwrap();
    assert!(to - from > 20_000, "the window is longer than the 20 s the playlist lists: {}", to - from);
    rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_clip_is_cut_by_stream_copy_into_an_unguessable_published_folder() {
    let rig = rig();
    load(&rig);
    rig.live.switch(request("p:1")).await;
    assert!(buffered(&rig, 12).await);
    let (_, to) = rig.live.clip_buffer_info().unwrap();

    let cut = rig.live.clip_cut(to - 9000, to - 2000, "espn-1-but-2").await.unwrap();

    let file = rig.live.public_dir().join(&cut.path);
    assert!(file.is_file());
    let parts: Vec<&str> = cut.path.split('/').collect();
    assert_eq!((parts[0], parts[2]), (CLIPS_DIR, "espn-1-but-2.mp4"));
    assert!(parts[1].len() >= 16, "{}", parts[1]);
    let seconds = ffprobe_duration(&file);
    assert!((seconds - cut.duration_ms as f64 / 1000.0).abs() < 1.0, "{seconds} vs {}", cut.duration_ms);
    assert!(cut.duration_ms >= 7000 && cut.duration_ms <= 10_000, "{}", cut.duration_ms);
    assert!(std::fs::read_dir(rig.live.public_dir().join(CLIPS_DIR)).unwrap().count() == 1);
    rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_clip_survives_a_zap_and_the_old_run_stays_cuttable() {
    let rig = rig();
    load(&rig);
    rig.live.switch(request("p:1")).await;
    assert!(buffered(&rig, 10).await);
    let (_, before_zap) = rig.live.clip_buffer_info().unwrap();
    let old_folder = rig.live.lock().run.as_ref().unwrap().folder.clone();
    rig.live.switch(request("p:3")).await;
    rig.live.tick_blocking_buffer();

    let cut = rig.live.clip_cut(before_zap - 8000, before_zap - 2000, "before-the-zap").await.unwrap();

    assert!(rig.live.public_dir().join(&cut.path).is_file());
    assert!(!rig.live.public_dir().join(old_folder).exists(), "the old run is gone from the published folder");
    rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_bad_name_a_bad_window_or_a_window_outside_the_buffer_is_refused_with_its_reason() {
    let rig = rig();
    load(&rig);
    rig.live.switch(request("p:1")).await;
    assert!(buffered(&rig, 6).await);
    let (from, _) = rig.live.clip_buffer_info().unwrap();

    assert_eq!(rig.live.clip_cut(from, from + 3000, "../etc").await, Err(ClipError::InvalidName));
    assert_eq!(rig.live.clip_cut(from, from + 241_000, "long-clip").await, Err(ClipError::InvalidWindow));
    assert_eq!(rig.live.clip_cut(5, 4, "backwards").await, Err(ClipError::InvalidWindow));
    assert_eq!(rig.live.clip_cut(from - 900_000, from - 890_000, "too-old").await, Err(ClipError::OutOfBuffer));
    rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn clips_outlive_the_module_but_the_buffer_does_not() {
    let rig = rig();
    load(&rig);
    rig.live.switch(request("p:1")).await;
    assert!(buffered(&rig, 8).await);
    let (_, to) = rig.live.clip_buffer_info().unwrap();
    let cut = rig.live.clip_cut(to - 6000, to - 1000, "kept-clip").await.unwrap();

    rig.live.shutdown();

    assert!(rig.live.public_dir().join(&cut.path).is_file());
    assert!(!rig.live.buffer_path().exists());
    assert_eq!(rig.live.clip_buffer_info(), None);
}
