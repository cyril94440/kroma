use std::time::Duration;

use super::*;

const COPY: StreamMode = StreamMode::new(VideoMode::Copy, AudioMode::Copy);

fn engine(root: &Path) -> HlsEngine {
    HlsEngine::new(root.join("hls"), 2, 0)
}

fn asked<'a>(input: &'a MediaInput, anchor: u64) -> Asked<'a> {
    Asked {
        item_id: "it1",
        input,
        opener: None,
        audio: 0,
        mode: COPY,
        anchor,
        source: Some((320, 180)),
        video: None,
        track: None,
    }
}

async fn remux(engine: &HlsEngine, input: &Path) -> String {
    let input = MediaInput::Local(input.to_path_buf());
    engine
        .master(asked(&input, 0))
        .await
        .expect("a playlist out of a file ffmpeg can read")
        .0
}

async fn finished_playlist(engine: &HlsEngine) -> String {
    for _ in 0..200 {
        let Some((bytes, _)) = engine.file("it1", COPY, 0, 0, "index.m3u8").await else {
            panic!("the session went away while its playlist was being read");
        };
        let text = String::from_utf8(bytes).expect("a text playlist");
        if text.contains("#EXT-X-ENDLIST") {
            return text;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("the remux never finished the clip");
}

#[tokio::test]
async fn the_length_a_file_carries_is_read_once_and_kept() {
    let dir = kroma_testing::temp_dir("hls-duration");
    let input = kroma_testing::clip(dir.path());
    let engine = engine(dir.path());
    let path = MediaInput::Local(input.clone());

    let first = engine.input_duration_ms(&path).await;
    let again = engine.input_duration_ms(&path).await;

    assert_eq!(first, again);
    assert!(
        matches!(first, Some(ms) if (4_000..6_000).contains(&ms)),
        "read {first:?} off a five second clip"
    );
}

#[tokio::test]
async fn a_file_nothing_can_open_has_no_length_and_is_not_probed_twice() {
    let dir = kroma_testing::temp_dir("hls-no-duration");
    let engine = engine(dir.path());

    let first = engine.input_duration_ms(&MediaInput::Local("/nowhere/absent.mkv".into())).await;
    let again = engine.input_duration_ms(&MediaInput::Local("/nowhere/absent.mkv".into())).await;

    assert_eq!(first, None);
    assert_eq!(again, None);
}

#[tokio::test]
async fn what_a_session_wrote_is_what_the_cache_weighs() {
    let dir = kroma_testing::temp_dir("hls-bytes");
    let input = kroma_testing::clip(dir.path());
    let engine = engine(dir.path());

    remux(&engine, &input).await;

    assert!(engine.cache_bytes() > 0);
}

#[tokio::test]
async fn the_disk_budget_is_retuned_without_restarting_the_engine() {
    let dir = kroma_testing::temp_dir("hls-budget");
    let engine = engine(dir.path());

    engine.set_cache_budget(4 * 1024 * 1024);
    engine.spawn_reaper();

    assert_eq!(engine.cache_bytes(), 0);
}

#[tokio::test]
async fn a_running_remux_says_what_it_is_producing_and_on_what_silicon() {
    let dir = kroma_testing::temp_dir("hls-live");
    let input = kroma_testing::clip(dir.path());
    let engine = engine(dir.path());

    remux(&engine, &input).await;

    let live = engine.live().await;
    assert_eq!(live.len(), 1);
    assert_eq!(live[0].item_id, "it1");
    assert_eq!(live[0].video, "copy");
    assert_eq!(live[0].audio, "copy");
    assert_eq!(live[0].source_width, Some(320));
    assert!(!live[0].transcodes_video);
}

#[test]
fn the_pipeline_this_host_settled_on_comes_with_the_reason_for_it() {
    let hardware = HlsEngine::hardware();

    assert!(!hardware.reason.is_empty());
    assert_eq!(hardware.accel, HlsEngine::hardware().accel);
}

#[tokio::test]
async fn a_child_name_reaching_out_of_the_session_is_refused_outright() {
    let dir = kroma_testing::temp_dir("hls-escape");
    let input = kroma_testing::clip(dir.path());
    let engine = engine(dir.path());
    remux(&engine, &input).await;

    let escaped = engine.file("it1", COPY, 0, 0, "../../etc/passwd").await;

    assert!(escaped.is_none());
}

#[tokio::test]
async fn a_finished_remux_is_seekable_vod_rather_than_a_live_edge() {
    let dir = kroma_testing::temp_dir("hls-endlist");
    let input = kroma_testing::clip(dir.path());
    let engine = engine(dir.path());
    remux(&engine, &input).await;

    let finished = finished_playlist(&engine).await;

    assert!(finished.contains("#EXT-X-TARGETDURATION"));
    assert!(finished.contains("seg_00000.m4s"));
}

#[tokio::test]
async fn a_segment_served_out_of_a_session_knows_what_it_is() {
    let dir = kroma_testing::temp_dir("hls-segment");
    let input = kroma_testing::clip(dir.path());
    let engine = engine(dir.path());
    remux(&engine, &input).await;
    finished_playlist(&engine).await;

    let init = engine.file("it1", COPY, 0, 0, "init.mp4").await;
    let segment = engine.file("it1", COPY, 0, 0, "seg_00000.m4s").await;

    assert_eq!(init.map(|(_, ct)| ct), Some("video/mp4"));
    assert_eq!(segment.map(|(_, ct)| ct), Some("video/iso.segment"));
}

fn session_dirs(root: &Path) -> usize {
    std::fs::read_dir(root.join("hls"))
        .map(|entries| entries.flatten().filter(|e| e.path().is_dir()).count())
        .unwrap_or(0)
}

#[tokio::test]
async fn a_re_anchor_into_a_session_already_running_is_answered_out_of_it() {
    let dir = kroma_testing::temp_dir("hls-reanchor");
    let input = kroma_testing::clip(dir.path());
    let engine = engine(dir.path());
    remux(&engine, &input).await;
    finished_playlist(&engine).await;

    let (playlist, start) = engine
        .master(asked(&MediaInput::Local(input.clone()), 2))
        .await
        .expect("the anchor sits inside what the running session has produced");

    assert_eq!(session_dirs(dir.path()), 1, "no second ffmpeg was started");
    assert_eq!(start, 0.0, "the client is told where the stream it gets begins");
    assert!(playlist.contains("seg_00000.m4s"));
}

#[tokio::test]
async fn a_title_behind_a_url_is_remuxed_from_the_network() {
    let dir = kroma_testing::temp_dir("hls-remote");
    let clip = kroma_testing::clip(dir.path());
    let server = kroma_testing::RangedServer::serve(&clip);
    let input = crate::input::relayed_at(server.port());
    let engine = engine(dir.path());

    let (playlist, start) = engine
        .master(asked(&input, 0))
        .await
        .expect("a playlist out of a URL ffmpeg can read");

    assert!(playlist.contains("#EXT-X-TARGETDURATION"));
    assert_eq!(start, 0.0);
    assert!(server.heard().iter().any(|r| r.range.is_some()), "{:?}", server.heard());
    let session = dir.path().join("hls").read_dir().unwrap().next().unwrap().unwrap().path();
    assert!(!session.join("ffmpeg.log").exists(), "a URL's errors are not kept on disk");
}

#[tokio::test]
async fn a_seek_into_a_url_starts_on_a_keyframe_read_over_the_network() {
    let dir = kroma_testing::temp_dir("hls-remote-seek");
    let clip = kroma_testing::clip(dir.path());
    let server = kroma_testing::RangedServer::serve(&clip);
    let input = crate::input::relayed_at(server.port());

    let start = super::ffmpeg::keyframe_before(&input, 3.2).await;

    assert!((start - 3.0).abs() < 0.1, "{start}");
}
