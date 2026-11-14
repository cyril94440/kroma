use super::*;
use crate::hls::tonemap::Route;

const UHD: (u32, u32) = (3840, 2160);

fn truehd_beside(video: &VideoStream, mode: StreamMode, set: Option<&str>) -> StreamMode {
    mode.for_copied_picture(Some(video), Some("truehd"), Some(8), set)
}

#[test]
fn truehd_beside_a_profile_7_copy_is_re_encoded_keeping_its_surround_bed() {
    let video = hdr_video(Some(HdrFormat::DolbyVision), Some(7), Some(6));

    let mode = truehd_beside(&video, copy(), Some("aac,eac3"));

    assert_eq!(mode.audio, AudioMode::Eac3);
    assert_eq!(mode.video, VideoMode::Copy);
}

#[test]
fn it_falls_to_stereo_aac_for_a_client_that_declared_nothing() {
    let video = hdr_video(Some(HdrFormat::DolbyVision), Some(7), Some(6));

    assert_eq!(truehd_beside(&video, copy(), None).audio, AudioMode::Aac);
}

#[test]
fn truehd_beside_a_profile_5_copy_is_re_encoded_unless_the_dolby_box_is_wanted() {
    let video = hdr_video(Some(HdrFormat::DolbyVision), Some(5), Some(0));
    let dolby = StreamMode::new(VideoMode::CopyDolby, AudioMode::Copy);

    assert_eq!(
        truehd_beside(&video, copy(), Some("aac")).audio,
        AudioMode::Aac
    );
    assert_eq!(truehd_beside(&video, dolby, Some("aac")), dolby);
}

#[test]
fn truehd_beside_every_other_picture_and_every_other_codec_is_left_alone() {
    let profile_8 = hdr_video(Some(HdrFormat::DolbyVision), Some(8), Some(1));
    let profile_7 = hdr_video(Some(HdrFormat::DolbyVision), Some(7), Some(6));

    assert_eq!(truehd_beside(&profile_8, copy(), Some("aac")), copy());
    assert_eq!(
        copy().for_copied_picture(Some(&profile_7), Some("dts"), Some(6), Some("aac")),
        copy()
    );
    assert_eq!(
        re_encoded().for_copied_picture(Some(&profile_7), Some("truehd"), Some(8), Some("aac")),
        re_encoded()
    );
}

fn at_1080() -> StreamMode {
    StreamMode::new(VideoMode::H264At(Rung::P1080), AudioMode::Aac)
}

#[test]
fn a_4k_tone_map_on_the_cpu_of_a_few_core_host_drops_to_720p() {
    let mode = at_1080().for_tone_map_cost_on(Some(UHD), Some(Route::Cpu), true, 4);

    assert_eq!(mode.video, VideoMode::H264At(Rung::P720));
}

#[test]
fn a_host_with_headroom_a_device_route_or_a_small_source_keeps_1080p() {
    assert_eq!(
        at_1080().for_tone_map_cost_on(Some(UHD), Some(Route::Cpu), true, 32),
        at_1080()
    );
    assert_eq!(
        at_1080().for_tone_map_cost_on(Some(UHD), Some(Route::Device), false, 2),
        at_1080()
    );
    assert_eq!(
        at_1080().for_tone_map_cost_on(Some((1920, 1080)), Some(Route::Cpu), true, 2),
        at_1080()
    );
}

#[test]
fn nothing_is_lowered_where_there_is_no_tone_map_or_no_measured_source() {
    assert_eq!(
        at_1080().for_tone_map_cost_on(Some(UHD), None, true, 2),
        at_1080()
    );
    assert_eq!(
        at_1080().for_tone_map_cost_on(None, Some(Route::Cpu), true, 2),
        at_1080()
    );
    assert_eq!(
        copy().for_tone_map_cost_on(Some(UHD), Some(Route::Cpu), true, 2),
        copy()
    );
}
