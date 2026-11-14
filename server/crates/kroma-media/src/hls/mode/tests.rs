use super::*;

const VIDEO_MODES: [VideoMode; 5] = [
    VideoMode::Copy,
    VideoMode::CopyDolby,
    VideoMode::H264,
    VideoMode::H264At(Rung::P1080),
    VideoMode::H264At(Rung::P720),
];
const AUDIO_MODES: [AudioMode; 6] = [
    AudioMode::Copy,
    AudioMode::Aac,
    AudioMode::AacStandard,
    AudioMode::AacNight,
    AudioMode::Eac3,
    AudioMode::Ac3,
];

#[test]
fn mode_tokens_round_trip() {
    for video in VIDEO_MODES {
        for audio in AUDIO_MODES {
            let mode = StreamMode::new(video, audio);
            assert_eq!(StreamMode::parse(&mode.token()), Some(mode));
        }
    }
    assert_eq!(StreamMode::parse("bogus"), None);
    assert_eq!(StreamMode::parse("h264-"), None);
    assert_eq!(StreamMode::parse("h264-bogus"), None);
}

#[test]
fn a_token_with_no_video_prefix_still_means_a_video_copy() {
    for (token, audio) in [
        ("copy", AudioMode::Copy),
        ("aac", AudioMode::Aac),
        ("aac-standard", AudioMode::AacStandard),
        ("aac-night", AudioMode::AacNight),
        ("eac3", AudioMode::Eac3),
        ("ac3", AudioMode::Ac3),
    ] {
        assert_eq!(
            StreamMode::parse(token),
            Some(StreamMode::new(VideoMode::Copy, audio))
        );
    }
}

#[test]
fn the_video_axis_is_a_prefix_on_the_audio_token() {
    assert_eq!(
        StreamMode::new(VideoMode::H264, AudioMode::Copy).token(),
        "h264-copy"
    );
    assert_eq!(
        StreamMode::new(VideoMode::H264, AudioMode::AacNight).token(),
        "h264-aac-night"
    );
}

#[test]
fn the_surround_targets_are_named_after_their_codec_and_carry_the_video_prefix() {
    assert_eq!(
        StreamMode::new(VideoMode::Copy, AudioMode::Eac3).token(),
        "eac3"
    );
    assert_eq!(
        StreamMode::new(VideoMode::H264At(Rung::P1080), AudioMode::Ac3).token(),
        "h264-1080-ac3"
    );
    assert_eq!(
        StreamMode::parse("h264-720-eac3"),
        Some(StreamMode::new(VideoMode::H264At(Rung::P720), AudioMode::Eac3))
    );
}

#[test]
fn the_surround_targets_transcode_without_a_loudness_filter() {
    for mode in [AudioMode::Eac3, AudioMode::Ac3] {
        assert!(mode.transcode());
        assert!(mode.filter_chain().is_none());
    }
}

#[test]
fn filtered_modes_transcode_with_a_chain() {
    assert!(!AudioMode::Copy.transcode());
    assert!(AudioMode::Aac.transcode());
    assert!(AudioMode::Aac.filter_chain().is_none());
    assert!(AudioMode::AacStandard.transcode());
    assert!(AudioMode::AacStandard
        .filter_chain()
        .unwrap()
        .contains("ratio=4"));
    assert!(AudioMode::AacNight
        .filter_chain()
        .unwrap()
        .contains("ratio=8"));
}

#[test]
fn a_dolby_copy_is_its_own_token_and_still_a_copy() {
    assert_eq!(
        StreamMode::new(VideoMode::CopyDolby, AudioMode::Copy).token(),
        "dv-copy"
    );
    assert_eq!(
        StreamMode::parse("dv-eac3"),
        Some(StreamMode::new(VideoMode::CopyDolby, AudioMode::Eac3))
    );
    assert!(VideoMode::CopyDolby.copies() && VideoMode::Copy.copies());
    assert!(!VideoMode::H264.copies() && !VideoMode::H264At(Rung::P720).copies());
}
