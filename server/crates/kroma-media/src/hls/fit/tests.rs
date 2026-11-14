use kroma_module_wire::{HdrFormat, VideoStream};

use super::*;

const fn copy() -> StreamMode {
    StreamMode::new(VideoMode::Copy, AudioMode::Copy)
}

const fn audio(audio: AudioMode) -> StreamMode {
    StreamMode::new(VideoMode::Copy, audio)
}

const fn re_encoded() -> StreamMode {
    StreamMode::new(VideoMode::H264, AudioMode::Copy)
}

// The Chromecast HD's own ceiling, which is square because its decoders
// declare one size limit for both axes.
const CHROMECAST_HD: (u32, u32) = (1920, 1920);

const AVPLAYER: &str = "hevc,h264";

#[test]
fn scales_a_source_larger_than_the_decoder_the_client_declared() {
    let scaled = copy().for_client_frame(Some((3840, 2160)), Some(CHROMECAST_HD));

    assert_eq!(scaled.video, VideoMode::H264At(Rung::P1080));
    assert_eq!(scaled.audio, AudioMode::Copy);
    assert_eq!(scaled.token(), "h264-1080-copy");
}

#[test]
fn scales_a_scope_frame_that_is_only_too_wide() {
    // 1604 rows clears a 1920 ceiling; 3840 columns do not.
    let scaled = copy().for_client_frame(Some((3840, 1604)), Some(CHROMECAST_HD));

    assert_eq!(scaled.video, VideoMode::H264At(Rung::P1080));
}

#[test]
fn leaves_a_picture_the_client_can_already_take() {
    assert_eq!(
        copy().for_client_frame(Some((1920, 1080)), Some(CHROMECAST_HD)),
        copy()
    );
    assert_eq!(copy().for_client_frame(Some((3840, 2160)), None), copy());
    assert_eq!(copy().for_client_frame(None, Some(CHROMECAST_HD)), copy());
}

#[test]
fn overrides_a_full_size_re_encode_too_because_it_is_just_as_unplayable() {
    let full = StreamMode::new(VideoMode::H264, AudioMode::Aac);

    let scaled = full.for_client_frame(Some((3840, 2160)), Some(CHROMECAST_HD));

    assert_eq!(scaled.video, VideoMode::H264At(Rung::P1080));
    assert_eq!(scaled.audio, AudioMode::Aac);
}

#[test]
fn drops_to_the_largest_rung_a_smaller_decoder_still_holds() {
    assert_eq!(
        copy()
            .for_client_frame(Some((3840, 2160)), Some((1280, 800)))
            .video,
        VideoMode::H264At(Rung::P720)
    );
    assert_eq!(
        copy()
            .for_client_frame(Some((3840, 2160)), Some((640, 480)))
            .video,
        VideoMode::Copy
    );
}

#[test]
fn copy_becomes_aac_when_the_client_cannot_decode_the_codec() {
    // The AC-3-only Android TV case: a copy request the device can't play.
    assert_eq!(
        copy().for_client_audio(Some("ac3"), Some("aac")),
        audio(AudioMode::Aac)
    );
}

#[test]
fn copy_stays_copy_when_the_client_can_decode_or_passthrough() {
    assert_eq!(
        copy().for_client_audio(Some("ac3"), Some("aac,ac3,eac3")),
        copy()
    );
    assert_eq!(
        copy().for_client_audio(Some("AC3"), Some("aac, ac3")),
        copy()
    );
}

#[test]
fn no_declared_capability_leaves_the_requested_mode_untouched() {
    assert_eq!(copy().for_client_audio(Some("ac3"), None), copy());
}

#[test]
fn an_empty_capability_set_transcodes_every_known_codec() {
    assert_eq!(
        copy().for_client_audio(Some("aac"), Some("")),
        audio(AudioMode::Aac)
    );
}

#[test]
fn an_unknown_codec_is_never_forced_to_transcode() {
    assert_eq!(copy().for_client_audio(None, Some("aac")), copy());
    assert_eq!(copy().for_client_audio(None, Some("")), copy());
}

#[test]
fn a_transcode_or_filter_request_is_never_second_guessed() {
    for mode in [AudioMode::Aac, AudioMode::AacStandard, AudioMode::AacNight] {
        assert_eq!(
            audio(mode).for_client_audio(Some("ac3"), Some("")),
            audio(mode)
        );
    }
}

#[test]
fn video_copy_becomes_h264_when_the_client_cannot_decode_the_codec() {
    // The HEVC-Main10-on-an-8-bit-decoder case, declared as "I decode H.264 only".
    assert_eq!(
        copy().for_client_video(Some("hevc"), None, Some("h264")),
        re_encoded()
    );
}

#[test]
fn video_stays_a_copy_when_the_declared_set_holds_the_codec() {
    assert_eq!(
        copy().for_client_video(Some("hevc"), None, Some("h264,hevc,av1")),
        copy()
    );
    assert_eq!(
        copy().for_client_video(Some("HEVC"), None, Some("h264, hevc")),
        copy()
    );
}

#[test]
fn a_client_that_declares_no_video_never_pays_for_a_transcode() {
    assert_eq!(copy().for_client_video(Some("hevc"), None, None), copy());
    assert_eq!(copy().for_client_video(Some("vp9"), None, None), copy());
    assert_eq!(
        copy().for_client_video(Some("h264"), Some(10), None),
        copy()
    );
}

#[test]
fn an_empty_video_set_transcodes_every_known_codec() {
    assert_eq!(
        copy().for_client_video(Some("h264"), None, Some("")),
        re_encoded()
    );
}

#[test]
fn an_unknown_video_codec_is_never_forced_to_transcode() {
    assert_eq!(copy().for_client_video(None, None, Some("h264")), copy());
    assert_eq!(copy().for_client_video(None, Some(10), Some("")), copy());
}

#[test]
fn an_h264_request_is_never_second_guessed() {
    let forced = re_encoded();
    assert_eq!(
        forced.for_client_video(Some("h264"), None, Some("h264")),
        forced
    );
    assert_eq!(forced.for_client_video(Some("hevc"), None, None), forced);
    assert_eq!(
        forced.for_client_video(Some("h264"), Some(10), Some("h264")),
        forced
    );
}

#[test]
fn ten_bit_h264_is_re_encoded_for_a_client_that_declared_h264_alone() {
    assert_eq!(
        copy().for_client_video(Some("h264"), Some(10), Some(AVPLAYER)),
        re_encoded()
    );
    assert_eq!(
        copy().for_client_video(Some("h264"), Some(12), Some(AVPLAYER)),
        re_encoded()
    );
}

#[test]
fn ten_bit_h264_stays_a_copy_for_a_client_that_declared_it_decodes_one() {
    assert_eq!(
        copy().for_client_video(Some("h264"), Some(10), Some("h264,h26410bit")),
        copy()
    );
    assert_eq!(
        copy().for_client_video(Some("H264"), Some(10), Some("h264, H26410bit")),
        copy()
    );
}

#[test]
fn eight_bit_h264_stays_a_copy_for_a_client_that_declared_only_h264() {
    assert_eq!(
        copy().for_client_video(Some("h264"), Some(8), Some(AVPLAYER)),
        copy()
    );
}

#[test]
fn a_depth_nothing_read_keeps_the_copy_the_codec_name_earned() {
    assert_eq!(
        copy().for_client_video(Some("h264"), None, Some(AVPLAYER)),
        copy()
    );
}

#[test]
fn the_deep_declaration_does_not_stand_in_for_plain_h264() {
    assert_eq!(
        copy().for_client_video(Some("h264"), Some(8), Some("h26410bit")),
        re_encoded()
    );
}

#[test]
fn ten_bit_hevc_is_still_matched_on_its_name_alone() {
    assert_eq!(
        copy().for_client_video(Some("hevc"), Some(10), Some("hevc")),
        copy()
    );
}

#[test]
fn the_two_axes_move_independently() {
    let both = copy()
        .for_client_audio(Some("dts"), Some("aac"))
        .for_client_video(Some("hevc"), None, Some("h264"));
    assert_eq!(both, StreamMode::new(VideoMode::H264, AudioMode::Aac));
    assert_eq!(both.token(), "h264-aac");
    assert_eq!(
        audio(AudioMode::AacNight).for_client_video(Some("hevc"), None, Some("h264")),
        StreamMode::new(VideoMode::H264, AudioMode::AacNight)
    );
    assert_eq!(
        re_encoded().for_client_audio(Some("dts"), Some("aac")),
        StreamMode::new(VideoMode::H264, AudioMode::Aac)
    );
}

#[test]
fn a_video_declaration_alone_never_moves_the_audio_axis() {
    assert_eq!(
        copy()
            .for_client_video(Some("hevc"), None, Some("h264"))
            .audio,
        AudioMode::Copy
    );
    assert_eq!(
        copy().for_client_audio(Some("dts"), Some("aac")).video,
        VideoMode::Copy
    );
}

fn hdr_video(
    format: Option<HdrFormat>,
    profile: Option<u32>,
    compatibility: Option<u8>,
) -> VideoStream {
    VideoStream {
        codec: "hevc".into(),
        width: Some(3840),
        height: Some(2160),
        hdr: format.is_some(),
        bit_depth: Some(10),
        hdr_format: format,
        dolby_vision_profile: profile,
        dolby_vision_level: None,
        dolby_vision_compatibility: compatibility,
        dolby_vision_enhancement_layer: false,
        color: None,
    }
}

fn hdr(video: &VideoStream, shows: Option<&str>, can_tone_map: bool) -> VideoMode {
    copy().for_client_hdr(Some(video), shows, can_tone_map).video
}

const HDR10: fn() -> VideoStream = || hdr_video(Some(HdrFormat::Hdr10), None, None);
const HLG: fn() -> VideoStream = || hdr_video(Some(HdrFormat::Hlg), None, None);
const PROFILE_5: fn() -> VideoStream = || hdr_video(Some(HdrFormat::DolbyVision), Some(5), Some(0));
const PROFILE_8_1: fn() -> VideoStream =
    || hdr_video(Some(HdrFormat::DolbyVision), Some(8), Some(1));
const PROFILE_8_2: fn() -> VideoStream =
    || hdr_video(Some(HdrFormat::DolbyVision), Some(8), Some(2));

#[test]
fn a_client_that_declares_no_hdr_formats_keeps_every_copy_it_asked_for() {
    for video in [HDR10(), HLG(), PROFILE_5(), PROFILE_8_1()] {
        assert_eq!(hdr(&video, None, true), VideoMode::Copy);
    }
}

#[test]
fn an_sdr_only_client_gets_every_hdr_source_re_encoded() {
    for video in [HDR10(), HLG(), PROFILE_5(), PROFILE_8_1()] {
        assert_eq!(hdr(&video, Some(""), true), VideoMode::H264);
    }
}

#[test]
fn an_sdr_source_and_an_sdr_compatible_dolby_vision_one_are_never_re_encoded_for_hdr() {
    let sdr = hdr_video(None, None, None);

    assert_eq!(hdr(&sdr, Some(""), true), VideoMode::Copy);
    assert_eq!(hdr(&PROFILE_8_2(), Some(""), true), VideoMode::Copy);
}

#[test]
fn profile_5_is_sent_only_to_a_client_that_declares_dolby_vision() {
    assert_eq!(hdr(&PROFILE_5(), Some("hdr10,hlg"), true), VideoMode::H264);
    assert_eq!(hdr(&PROFILE_5(), Some("hdr10,dv"), true), VideoMode::Copy);
}

#[test]
fn a_dolby_vision_stream_with_a_hdr10_base_plays_as_hdr10_without_dv() {
    assert_eq!(hdr(&PROFILE_8_1(), Some("hdr10"), true), VideoMode::Copy);
    assert_eq!(hdr(&PROFILE_8_1(), Some("hlg"), true), VideoMode::H264);
    assert_eq!(hdr(&PROFILE_8_1(), Some("dv"), true), VideoMode::Copy);
}

#[test]
fn a_dolby_vision_stream_with_an_hlg_base_plays_as_hlg_without_dv() {
    let profile_8_4 = hdr_video(Some(HdrFormat::DolbyVision), Some(8), Some(4));

    assert_eq!(hdr(&profile_8_4, Some("hlg"), true), VideoMode::Copy);
    assert_eq!(hdr(&profile_8_4, Some("hdr10"), true), VideoMode::H264);
}

#[test]
fn hdr10_plus_plays_on_a_client_that_only_declares_hdr10() {
    let plus = hdr_video(Some(HdrFormat::Hdr10Plus), None, None);

    assert_eq!(hdr(&plus, Some("hdr10"), true), VideoMode::Copy);
    assert_eq!(hdr(&plus, Some("hlg"), true), VideoMode::H264);
}

#[test]
fn each_format_needs_its_own_declaration() {
    assert_eq!(hdr(&HDR10(), Some("hlg,dv"), true), VideoMode::H264);
    assert_eq!(hdr(&HLG(), Some("hdr10,dv"), true), VideoMode::H264);
    assert_eq!(hdr(&HDR10(), Some(" HDR10 , hlg"), true), VideoMode::Copy);
}

#[test]
fn no_re_encode_is_ordered_where_it_could_not_come_out_in_sdr() {
    for video in [HDR10(), PROFILE_5()] {
        assert_eq!(hdr(&video, Some(""), false), VideoMode::Copy);
    }
}

#[test]
fn only_a_copy_is_overridden_for_hdr() {
    let rung = StreamMode::new(VideoMode::H264At(Rung::P720), AudioMode::Aac);

    assert_eq!(rung.for_client_hdr(Some(&HDR10()), Some(""), true), rung);
    assert_eq!(
        re_encoded().for_client_hdr(Some(&HDR10()), Some(""), true),
        re_encoded()
    );
    assert_eq!(copy().for_client_hdr(None, Some(""), true), copy());
}

fn surround(codec: &str, channels: u32, set: &str) -> AudioMode {
    copy()
        .for_client_track(Some(codec), Some(channels), Some(set))
        .audio
}

#[test]
fn an_undecodable_surround_track_keeps_its_bed_as_eac3_when_the_client_declares_it() {
    assert_eq!(surround("truehd", 8, "aac,eac3"), AudioMode::Eac3);
    assert_eq!(surround("dts", 6, "aac,ac3,eac3"), AudioMode::Eac3);
    assert_eq!(surround("flac", 6, "eac3"), AudioMode::Eac3);
}

#[test]
fn it_falls_back_on_ac3_when_that_is_the_only_dolby_codec_declared() {
    assert_eq!(surround("dts", 6, "aac,ac3"), AudioMode::Ac3);
}

#[test]
fn it_falls_back_on_stereo_aac_when_the_client_declares_neither() {
    assert_eq!(surround("dts", 6, "aac"), AudioMode::Aac);
    assert_eq!(surround("dts", 6, ""), AudioMode::Aac);
}

#[test]
fn a_stereo_or_unprobed_track_is_never_upmixed_into_surround() {
    assert_eq!(surround("flac", 2, "aac,eac3"), AudioMode::Aac);
    assert_eq!(surround("dts", 1, "eac3"), AudioMode::Aac);
    assert_eq!(
        copy()
            .for_client_track(Some("dts"), None, Some("eac3"))
            .audio,
        AudioMode::Aac
    );
}

#[test]
fn a_codec_the_client_declares_is_copied_whatever_its_channel_count() {
    assert_eq!(surround("eac3", 6, "aac,eac3"), AudioMode::Copy);
    assert_eq!(surround("truehd", 8, "truehd,eac3"), AudioMode::Copy);
}

#[test]
fn the_older_audio_entry_point_still_means_stereo_aac() {
    assert_eq!(
        copy()
            .for_client_audio(Some("dts"), Some("aac,eac3"))
            .audio,
        AudioMode::Aac
    );
}

#[test]
fn only_a_copy_request_is_turned_into_a_surround_transcode() {
    let night = audio(AudioMode::AacNight);

    assert_eq!(
        night.for_client_track(Some("dts"), Some(6), Some("aac,eac3")),
        night
    );
    assert_eq!(
        audio(AudioMode::Eac3).for_client_track(Some("dts"), Some(6), Some("aac")),
        audio(AudioMode::Eac3)
    );
}

const AVPLAY_COPY_SET: &str = "aac,ac3,eac3,mp3";
const MOBILE_COPY_SET: &str = "aac,ac3,eac3";

#[test]
fn the_sets_the_clients_declare_keep_the_surround_bed_of_every_undecodable_track() {
    for set in [AVPLAY_COPY_SET, MOBILE_COPY_SET] {
        assert_eq!(surround("truehd", 8, set), AudioMode::Eac3, "{set}");
        assert_eq!(surround("dts", 6, set), AudioMode::Eac3, "{set}");
    }
    assert_eq!(surround("dts", 6, "aac,ac3,mp3"), AudioMode::Ac3);
    assert_eq!(surround("truehd", 8, "aac"), AudioMode::Aac);
    assert_eq!(surround("aac", 2, "aac"), AudioMode::Copy);
}

#[test]
fn a_re_encoded_picture_keeps_the_surround_bed_the_same_request_earned() {
    let sdr_only = copy()
        .for_client_track(Some("truehd"), Some(8), Some(MOBILE_COPY_SET))
        .for_client_hdr(Some(&hdr_video(Some(HdrFormat::Hdr10), None, None)), Some(""), true);

    assert_eq!(
        sdr_only,
        StreamMode::new(VideoMode::H264, AudioMode::Eac3)
    );
    assert_eq!(sdr_only.token(), "h264-eac3");
}
