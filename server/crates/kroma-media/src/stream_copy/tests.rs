use super::*;

fn hevc(profile: Option<u32>) -> VideoStream {
    VideoStream {
        codec: "hevc".into(),
        width: Some(3840),
        height: Some(2160),
        hdr: true,
        bit_depth: Some(10),
        hdr_format: None,
        dolby_vision_profile: profile,
        dolby_vision_level: None,
        dolby_vision_compatibility: None,
        dolby_vision_enhancement_layer: false,
        color: None,
    }
}

fn of(codec: &str) -> VideoStream {
    VideoStream {
        codec: codec.into(),
        ..hevc(None)
    }
}

#[test]
fn plain_hevc_is_tagged_hvc1_and_asks_for_nothing_else() {
    assert_eq!(video_args(Some(&hevc(None))), ["-tag:v", "hvc1"]);
    assert_eq!(strictness(Some(&hevc(None)), Some("eac3")), None);
}

#[test]
fn a_codec_other_than_hevc_gets_no_tag() {
    for codec in ["h264", "av1", "vp9"] {
        assert!(video_args(Some(&of(codec))).is_empty(), "{codec}");
    }
    assert!(video_args(None).is_empty());
}

#[test]
fn dolby_vision_profile_5_is_dvh1_and_writes_its_configuration_box() {
    assert_eq!(video_args(Some(&hevc(Some(5)))), ["-tag:v", "dvh1"]);
    assert_eq!(strictness(Some(&hevc(Some(5))), None), Some("unofficial"));
}

#[test]
fn dolby_vision_profile_8_stays_hvc1_with_its_dvvc_box() {
    assert_eq!(video_args(Some(&hevc(Some(8)))), ["-tag:v", "hvc1"]);
    assert_eq!(strictness(Some(&hevc(Some(8))), None), Some("unofficial"));
}

#[test]
fn a_single_track_profile_7_loses_its_enhancement_layer_and_its_box() {
    let args = video_args(Some(&hevc(Some(7))));

    assert_eq!(
        args,
        ["-tag:v", "hvc1", "-bsf:v", "filter_units=remove_types=62|63"]
    );
    assert_eq!(strictness(Some(&hevc(Some(7))), None), None);
}

#[test]
fn truehd_needs_the_experimental_level_and_it_covers_the_dolby_box() {
    assert_eq!(strictness(None, Some("truehd")), Some("-2"));
    assert_eq!(
        strictness(Some(&hevc(Some(8))), Some("truehd")),
        Some("-2")
    );
}

#[test]
fn the_other_audio_codecs_copy_without_a_strictness_flag() {
    for codec in ["aac", "ac3", "eac3", "dts", "flac", "opus"] {
        assert_eq!(strictness(None, Some(codec)), None, "{codec}");
    }
}
