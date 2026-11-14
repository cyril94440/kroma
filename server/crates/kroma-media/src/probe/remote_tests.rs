use kroma_primitives::input::MediaInput;
use kroma_testing::RangedServer;

use super::run::{probe_duration_ms, probe_input, probe_outcome, Probed};

fn served_clip() -> (kroma_testing::TempDir, RangedServer, MediaInput) {
    let dir = kroma_testing::temp_dir("probe-remote");
    let clip = kroma_testing::clip(dir.path());
    let server = RangedServer::serve(&clip);
    let input = crate::input::relayed_at(server.port());
    (dir, server, input)
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
fn a_title_the_host_refuses_is_a_network_failure_to_try_again_and_never_unreadable() {
    let (_dir, server, input) = served_clip();
    server.fail_with(403);

    let probed = probe_outcome(&input, "mkv", true);

    assert!(matches!(probed, Probed::Network), "{probed:?}");
    assert!(probe_input(&input, "mkv", true).is_none(), "no verdict is stored");
}

#[test]
fn bytes_that_arrive_whole_and_are_not_a_video_are_unreadable() {
    let dir = kroma_testing::temp_dir("probe-remote-junk");
    let junk = dir.path().join("clip.mp4");
    std::fs::write(&junk, vec![0x41u8; 200_000]).unwrap();
    let server = RangedServer::serve(&junk);
    let input = crate::input::relayed_at(server.port());

    let Probed::Done(result) = probe_outcome(&input, "mkv", true) else {
        panic!("the file was fetched and is a verdict");
    };

    let reason = result.unreadable.expect("a fetched, undecodable file is unreadable");
    assert!(!reason.contains("secret") && !reason.contains("grant"), "{reason}");
}

#[test]
fn without_ffprobe_the_guess_follows_the_real_container_not_the_link() {
    let (_dir, _server, input) = served_clip();

    let guessed = probe_input(&input, "WEBM", false).expect("a guess");

    assert_eq!(guessed.video.unwrap().codec, "vp9");
}
