use kroma_primitives::input::{MediaInput, RemoteUrl};
use kroma_testing::RangedServer;

use super::run::{probe_duration_ms, probe_input};

fn served_clip() -> (kroma_testing::TempDir, RangedServer, MediaInput) {
    let dir = kroma_testing::temp_dir("probe-remote");
    let clip = kroma_testing::clip(dir.path());
    let server = RangedServer::serve(&clip);
    let url = RemoteUrl::parse_insecure(&server.url("clip.mp4")).unwrap();
    (dir, server, MediaInput::Remote(url))
}

#[test]
fn a_title_behind_a_url_is_probed_over_range_requests() {
    let (_dir, server, input) = served_clip();

    let result = probe_input(&input, "mkv", true).expect("ffprobe answered");

    assert_eq!(result.unreadable, None);
    assert_eq!(result.video.as_ref().map(|v| v.codec.as_str()), Some("h264"));
    assert!(result.audio.is_some());
    assert!(matches!(result.duration_ms, Some(ms) if (4_000..6_000).contains(&ms)));
    assert!(server.heard().iter().all(|r| r.method == "GET" || r.method == "HEAD"));
}

#[test]
fn the_length_of_a_title_behind_a_url_is_read_from_its_header() {
    let (_dir, _server, input) = served_clip();

    let ms = probe_duration_ms(&input);

    assert!(matches!(ms, Some(ms) if (4_000..6_000).contains(&ms)), "{ms:?}");
}

#[test]
fn a_url_that_fails_is_unreadable_and_the_reason_never_holds_the_link() {
    let (_dir, server, input) = served_clip();
    server.fail_with(403);

    let result = probe_input(&input, "mkv", true).expect("a verdict");

    let reason = result.unreadable.expect("a refused read is unreadable");
    assert!(!reason.contains("hunter2") && !reason.contains("127.0.0.1:"), "{reason}");
}

#[test]
fn without_ffprobe_the_guess_follows_the_real_container_not_the_link() {
    let (_dir, _server, input) = served_clip();

    let guessed = probe_input(&input, "WEBM", false).expect("a guess");

    assert_eq!(guessed.video.unwrap().codec, "vp9");
}
