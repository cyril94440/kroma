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
    assert_eq!(video_args(Some(&hevc(None)), false), ["-tag:v", "hvc1"]);
    assert_eq!(strictness(Some(&hevc(None)), false, Some("eac3")), None);
}

#[test]
fn a_codec_other_than_hevc_gets_no_tag() {
    for codec in ["h264", "av1", "vp9"] {
        assert!(video_args(Some(&of(codec)), false).is_empty(), "{codec}");
    }
    assert!(video_args(None, false).is_empty());
}

#[test]
fn dolby_vision_profile_5_is_dvh1_and_writes_its_configuration_box_for_a_dolby_client() {
    assert_eq!(video_args(Some(&hevc(Some(5))), true), ["-tag:v", "dvh1"]);
    assert_eq!(
        strictness(Some(&hevc(Some(5))), true, None),
        Some("unofficial")
    );
}

#[test]
fn dolby_vision_profile_5_for_any_other_client_is_plain_hevc_with_no_box() {
    assert_eq!(video_args(Some(&hevc(Some(5))), false), ["-tag:v", "hvc1"]);
    assert_eq!(strictness(Some(&hevc(Some(5))), false, None), None);
}

#[test]
fn dolby_vision_profile_8_stays_hvc1_with_its_dvvc_box_whatever_the_client_declared() {
    for dolby in [false, true] {
        assert_eq!(video_args(Some(&hevc(Some(8))), dolby), ["-tag:v", "hvc1"]);
        assert_eq!(
            strictness(Some(&hevc(Some(8))), dolby, None),
            Some("unofficial")
        );
    }
}

#[test]
fn a_single_track_profile_7_loses_its_enhancement_layer_and_its_box() {
    let args = video_args(Some(&hevc(Some(7))), false);

    assert_eq!(
        args,
        ["-tag:v", "hvc1", "-bsf:v", "filter_units=remove_types=62|63"]
    );
    assert_eq!(strictness(Some(&hevc(Some(7))), false, None), None);
}

#[test]
fn truehd_needs_the_experimental_level_and_it_covers_the_dolby_box() {
    assert_eq!(strictness(None, false, Some("truehd")), Some("-2"));
    assert_eq!(
        strictness(Some(&hevc(Some(8))), false, Some("truehd")),
        Some("-2")
    );
}

#[test]
fn the_other_audio_codecs_copy_without_a_strictness_flag() {
    for codec in ["aac", "ac3", "eac3", "dts", "flac", "opus"] {
        assert_eq!(strictness(None, false, Some(codec)), None, "{codec}");
        assert_eq!(
            strictness(Some(&hevc(Some(7))), false, Some(codec)),
            None,
            "{codec}"
        );
    }
}

#[test]
fn truehd_is_never_copied_beside_a_picture_whose_dolby_layers_were_stripped() {
    assert!(!can_copy_audio_beside(Some(&hevc(Some(7))), false, "truehd"));
    assert!(!can_copy_audio_beside(Some(&hevc(Some(5))), false, "truehd"));
}

#[test]
fn truehd_is_copied_where_the_dolby_box_it_writes_is_wanted_or_there_is_none() {
    assert!(can_copy_audio_beside(Some(&hevc(Some(5))), true, "truehd"));
    assert!(can_copy_audio_beside(Some(&hevc(Some(8))), false, "truehd"));
    assert!(can_copy_audio_beside(Some(&hevc(None)), false, "truehd"));
    assert!(can_copy_audio_beside(None, false, "truehd"));
}

#[test]
fn audio_that_needs_no_strictness_copies_beside_any_picture() {
    for codec in ["aac", "ac3", "eac3", "dts"] {
        assert!(
            can_copy_audio_beside(Some(&hevc(Some(7))), false, codec),
            "{codec}"
        );
    }
}
