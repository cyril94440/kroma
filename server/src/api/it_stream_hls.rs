use axum::http::StatusCode;

use crate::api::test_support::{demo_item_id, raw, seed_playable_item, test_app, TestApp};

// A decoder that stops at 1920 on both axes, which is how a Chromecast HD
// declares itself: one number for both, not a 16:9 shape.
const HD_ONLY: &str = "maxw=1920&maxh=1920";

async fn location(t: &TestApp, uri: &str) -> (StatusCode, String) {
    let (status, headers, _) = raw(&t.app, "GET", uri, Some(&t.token), None, &[]).await;
    let location = headers
        .get("location")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    (status, location)
}

// An HDR source this host tone-maps on the CPU is served one rung lower on a host
// with few cores, so the rung the test sees depends on the machine it runs on.
fn rungs(id: &str, audio: &str) -> Vec<String> {
    let mut modes = vec![format!("h264-1080-{audio}")];
    if kroma_media::hls::tonemap_filters().zimg {
        modes.push(format!("h264-720-{audio}"));
    }
    modes
        .into_iter()
        .map(|mode| format!("/api/items/{id}/hls/{mode}/0/0/index.m3u8"))
        .collect()
}

fn master(id: &str, mode: &str, query: &str) -> String {
    format!("/api/items/{id}/hls/{mode}/0/0/index.m3u8?{query}")
}

#[tokio::test]
async fn a_4k_source_is_redirected_to_a_rung_the_declared_decoder_holds() {
    let t = test_app();
    let id = demo_item_id("Blade Runner 2049"); // hevc 3840x2160

    let (status, location) = location(&t, &master(&id, "copy", HD_ONLY)).await;

    assert_eq!(status, StatusCode::TEMPORARY_REDIRECT);
    assert!(rungs(&id, "copy").contains(&location), "{location}");
}

#[tokio::test]
async fn a_scope_frame_that_is_only_too_wide_is_redirected_too() {
    let t = test_app();
    let id = demo_item_id("Sintel"); // av1 4096x1744: under 1920 tall, still too wide

    let (status, location) = location(&t, &master(&id, "copy", HD_ONLY)).await;

    assert_eq!(status, StatusCode::TEMPORARY_REDIRECT);
    assert!(location.contains("h264-1080-copy"), "{location}");
}

#[tokio::test]
async fn both_axes_move_in_one_redirect() {
    let t = test_app();
    let id = demo_item_id("Blade Runner 2049"); // truehd on track 0, hevc picture

    let uri = master(&id, "copy", &format!("{HD_ONLY}&copy=aac&video=h264"));
    let (status, location) = location(&t, &uri).await;

    assert_eq!(status, StatusCode::TEMPORARY_REDIRECT);
    assert!(rungs(&id, "aac").contains(&location), "{location}");
}

#[tokio::test]
async fn a_surround_track_the_client_cannot_decode_is_redirected_to_eac3_when_it_declares_it() {
    let t = test_app();
    let id = demo_item_id("Blade Runner 2049"); // truehd 7.1 on track 0

    let (status, location) = location(&t, &master(&id, "copy", "copy=aac,eac3")).await;

    assert_eq!(status, StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(location, format!("/api/items/{id}/hls/eac3/0/0/index.m3u8"));
}

#[tokio::test]
async fn the_redirect_to_a_surround_mode_is_not_redirected_again() {
    let t = test_app();
    let id = demo_item_id("Blade Runner 2049");

    let (status, _) = location(&t, &master(&id, "eac3", "")).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_client_declaring_no_hdr_systems_has_its_hdr_picture_tone_mapped_where_the_host_can() {
    let t = test_app();
    let id = demo_item_id("Blade Runner 2049"); // hevc HDR10

    let (status, location) = location(&t, &master(&id, "copy", "hdr=")).await;

    if kroma_media::hls::tonemap_filters().zimg {
        assert_eq!(status, StatusCode::TEMPORARY_REDIRECT);
        assert!(rungs(&id, "copy").contains(&location), "a tone-mapped picture is never full size: {location}");
    } else {
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
async fn a_client_declaring_hdr10_keeps_its_hdr10_copy() {
    let t = test_app();
    let id = demo_item_id("Blade Runner 2049");

    let (status, _) = location(&t, &master(&id, "copy", "hdr=hdr10,hlg")).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_client_that_declares_nothing_is_never_sent_to_a_re_encode() {
    let t = test_app();
    let id = demo_item_id("Blade Runner 2049");

    // No `maxw`/`maxh` and no codec sets: the requested mode stands, the handler
    // falls through to the session, and the demo item's path is not a real file.
    let (status, _) = location(&t, &master(&id, "copy", "")).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_picture_the_decoder_already_takes_is_left_alone() {
    let t = test_app();
    let id = demo_item_id("The Matrix"); // h264 1920x1080, inside the ceiling

    let (status, _) = location(&t, &master(&id, "copy", HD_ONLY)).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_mode_nothing_parses_is_refused_before_the_item_is_read() {
    let t = test_app();
    let id = demo_item_id("The Matrix");

    let (status, _) = location(&t, &master(&id, "h264-bogus", "")).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn an_item_that_does_not_exist_is_not_found() {
    let t = test_app();

    let (status, _) = location(&t, &master("deadbeef", "copy", "")).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

fn clip_bytes() -> Vec<u8> {
    let dir = kroma_testing::temp_dir("it-hls-clip");
    std::fs::read(kroma_testing::clip(dir.path())).expect("read the fixture clip")
}

fn child(id: &str, mode: &str, name: &str) -> String {
    format!("/api/items/{id}/hls/{mode}/0/0/{name}")
}

#[tokio::test]
async fn a_file_ffmpeg_can_read_is_remuxed_into_a_playlist_the_player_opens() {
    let t = test_app();
    let id = seed_playable_item(&t.state, "Clip", &clip_bytes());

    let (status, headers, _) = raw(
        &t.app,
        "GET",
        &master(&id, "copy", ""),
        Some(&t.token),
        None,
        &[],
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers.get("content-type").unwrap(),
        "application/vnd.apple.mpegurl"
    );
    assert_eq!(headers.get("x-hls-start").unwrap(), "0.000");
}

#[tokio::test]
async fn a_child_of_a_running_session_is_served_out_of_it() {
    let t = test_app();
    let id = seed_playable_item(&t.state, "Clip", &clip_bytes());
    raw(
        &t.app,
        "GET",
        &master(&id, "copy", ""),
        Some(&t.token),
        None,
        &[],
    )
    .await;

    let (status, headers, _) = raw(
        &t.app,
        "GET",
        &child(&id, "copy", "init.mp4"),
        Some(&t.token),
        None,
        &[],
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers.get("content-type").unwrap(), "video/mp4");
}

#[tokio::test]
async fn a_child_of_a_session_nobody_started_is_not_found() {
    let t = test_app();
    let id = demo_item_id("The Matrix");

    let (status, headers, _) = raw(
        &t.app,
        "GET",
        &child(&id, "copy", "seg_00000.m4s"),
        Some(&t.token),
        None,
        &[],
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(headers.get("cache-control").unwrap(), "no-store");
}

#[tokio::test]
async fn a_child_asked_for_under_a_mode_nothing_parses_is_refused() {
    let t = test_app();
    let id = demo_item_id("The Matrix");

    let (status, _, _) = raw(
        &t.app,
        "GET",
        &child(&id, "h264-bogus", "init.mp4"),
        Some(&t.token),
        None,
        &[],
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_child_of_an_item_the_viewer_cannot_reach_is_never_served() {
    let t = test_app();

    let (status, _, _) = raw(
        &t.app,
        "GET",
        &child("deadbeef", "copy", "init.mp4"),
        Some(&t.token),
        None,
        &[],
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_codec_redirect_keeps_the_selected_native_subtitle() {
    let t = test_app();
    let id = demo_item_id("Blade Runner 2049");
    let (status, location) = location(&t, &master(&id, "copy", "copy=aac&subtitle=2")).await;
    assert_eq!(status, StatusCode::TEMPORARY_REDIRECT);
    assert!(location.ends_with("/aac/0/0/index.m3u8?subtitle=2"));
}

#[tokio::test]
async fn native_subtitles_wrap_the_media_playlist_without_losing_its_anchor() {
    let t = test_app();
    let id = seed_playable_item(&t.state, "Native Clip", &clip_bytes());
    let (status, headers, _) = raw(
        &t.app,
        "GET",
        &master(&id, "copy", "subtitle=0"),
        Some(&t.token),
        None,
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers.get("x-hls-start").unwrap(), "0.000");
    let (_, playlist) = crate::api::test_support::text(
        &t.app,
        "GET",
        &master(&id, "copy", "subtitle=0"),
        Some(&t.token),
        None,
    )
    .await;
    assert!(playlist.contains("TYPE=SUBTITLES"), "{playlist}");
    assert!(playlist.contains(&format!("/api/items/{id}/hls/copy/0/0/index.m3u8\n")));
}
