use std::path::PathBuf;

use super::super::hwaccel::HwAccel;
use super::super::software::Effort;
use super::super::tonemap::{Filters, ToneMap, ToneSource};
use super::super::Rung;
use super::*;

const ZIMG: Filters = Filters {
    zimg: true,
    placebo: false,
};

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

fn track(codec: &str, channels: u32) -> AudioStream {
    AudioStream {
        index: 0,
        codec: codec.into(),
        channels: Some(channels),
        language: None,
        title: None,
        default: true,
        region: None,
        profile: None,
        layout: None,
        spatial: None,
    }
}

fn pipeline(accel: HwAccel, tonemap: bool) -> Pipeline {
    Pipeline {
        accel,
        effort: Effort::Quality,
        tonemap: tonemap.then(|| ToneMap::resolve(ToneSource::Pq, ZIMG).unwrap()),
    }
}

fn line(
    mode: StreamMode,
    video: Option<&VideoStream>,
    audio: Option<&AudioStream>,
    pipeline: Pipeline,
) -> String {
    let dir = PathBuf::from("/s");
    let input = MediaInput::Local(PathBuf::from("/m/film.mkv"));
    build(&Spec {
        input: &input,
        dir: &dir,
        audio: 0,
        mode,
        start_secs: 0.0,
        burst: false,
        pipeline,
        video,
        track: audio,
    })
    .iter()
    .map(|a| a.to_string_lossy().into_owned())
    .collect::<Vec<_>>()
    .join(" ")
}

fn copy(audio: AudioMode) -> StreamMode {
    StreamMode::new(VideoMode::Copy, audio)
}

fn h264(audio: AudioMode) -> StreamMode {
    StreamMode::new(VideoMode::H264At(Rung::P1080), audio)
}

fn software() -> Pipeline {
    pipeline(HwAccel::Software, false)
}

#[test]
fn copied_hevc_is_tagged_hvc1_for_apple_players() {
    let cmd = line(copy(AudioMode::Copy), Some(&hevc(None)), None, software());

    assert!(cmd.contains("-c:v copy -tag:v hvc1"), "{cmd}");
    assert!(!cmd.contains("-strict"), "{cmd}");
}

#[test]
fn a_video_that_is_not_hevc_copies_with_no_tag() {
    let mut h264_source = hevc(None);
    h264_source.codec = "h264".into();

    let cmd = line(copy(AudioMode::Copy), Some(&h264_source), None, software());

    assert!(cmd.contains("-c:v copy -c:a copy"), "{cmd}");
}

#[test]
fn an_unprobed_source_copies_exactly_as_before() {
    let cmd = line(copy(AudioMode::Copy), None, None, software());

    assert!(cmd.contains("-c:v copy -c:a copy"), "{cmd}");
}

#[test]
fn profile_5_copy_is_dvh1_and_writes_the_dolby_box() {
    let cmd = line(copy(AudioMode::Copy), Some(&hevc(Some(5))), None, software());

    assert!(cmd.contains("-c:v copy -tag:v dvh1"), "{cmd}");
    assert!(cmd.contains("-strict unofficial"), "{cmd}");
}

#[test]
fn profile_8_copy_keeps_hvc1_and_writes_the_dolby_box() {
    let cmd = line(copy(AudioMode::Copy), Some(&hevc(Some(8))), None, software());

    assert!(cmd.contains("-c:v copy -tag:v hvc1"), "{cmd}");
    assert!(cmd.contains("-strict unofficial"), "{cmd}");
}

#[test]
fn profile_7_copy_is_cut_down_to_its_hdr10_base_layer() {
    let cmd = line(copy(AudioMode::Copy), Some(&hevc(Some(7))), None, software());

    assert!(
        cmd.contains("-tag:v hvc1 -bsf:v filter_units=remove_types=62|63"),
        "{cmd}"
    );
    assert!(!cmd.contains("-strict"), "{cmd}");
}

#[test]
fn a_re_encode_of_a_dolby_vision_source_carries_none_of_the_copy_flags() {
    let cmd = line(
        h264(AudioMode::Aac),
        Some(&hevc(Some(5))),
        Some(&track("eac3", 6)),
        software(),
    );

    for flag in ["-tag:v", "-strict", "-bsf:v"] {
        assert!(!cmd.contains(flag), "{flag} in {cmd}");
    }
}

#[test]
fn copied_truehd_asks_for_the_experimental_level_without_which_ffmpeg_refuses_to_start() {
    let cmd = line(
        copy(AudioMode::Copy),
        Some(&hevc(None)),
        Some(&track("truehd", 8)),
        software(),
    );

    assert!(cmd.contains("-c:a copy -strict -2"), "{cmd}");
    assert_eq!(cmd.matches("-strict").count(), 1, "{cmd}");
}

#[test]
fn truehd_beside_a_dolby_vision_copy_takes_the_one_level_that_covers_both() {
    let cmd = line(
        copy(AudioMode::Copy),
        Some(&hevc(Some(8))),
        Some(&track("truehd", 8)),
        software(),
    );

    assert!(cmd.contains("-strict -2"), "{cmd}");
    assert!(!cmd.contains("unofficial"), "{cmd}");
}

#[test]
fn a_truehd_track_that_is_re_encoded_needs_no_experimental_level() {
    let cmd = line(
        copy(AudioMode::Eac3),
        None,
        Some(&track("truehd", 8)),
        software(),
    );

    assert!(!cmd.contains("-strict"), "{cmd}");
}

#[test]
fn eac3_keeps_a_surround_bed_at_640k() {
    let cmd = line(
        copy(AudioMode::Eac3),
        None,
        Some(&track("dts", 6)),
        software(),
    );

    assert!(cmd.contains("-c:a eac3 -ac 6 -b:a 640k"), "{cmd}");
}

#[test]
fn ac3_keeps_a_surround_bed_at_640k() {
    let cmd = line(
        copy(AudioMode::Ac3),
        None,
        Some(&track("truehd", 8)),
        software(),
    );

    assert!(cmd.contains("-c:a ac3 -ac 6 -b:a 640k"), "{cmd}");
}

#[test]
fn a_stereo_or_unprobed_track_stays_stereo_aac_whatever_the_surround_mode() {
    for audio in [Some(track("flac", 2)), Some(track("flac", 1)), None] {
        for mode in [AudioMode::Eac3, AudioMode::Ac3] {
            let cmd = line(copy(mode), None, audio.as_ref(), software());

            assert!(cmd.contains("-c:a aac -ac 2 -b:a 192k"), "{cmd}");
        }
    }
}

#[test]
fn the_loudness_modes_stay_stereo_aac_with_their_filter() {
    let cmd = line(
        copy(AudioMode::AacNight),
        None,
        Some(&track("eac3", 6)),
        software(),
    );

    assert!(cmd.contains("-af acompressor="), "{cmd}");
    assert!(cmd.contains("-c:a aac -ac 2 -b:a 192k"), "{cmd}");
}

#[test]
fn plain_aac_is_the_command_it_always_was() {
    let cmd = line(copy(AudioMode::Aac), None, Some(&track("dts", 6)), software());

    assert!(cmd.contains("-c:a aac -ac 2 -b:a 192k"), "{cmd}");
    assert!(!cmd.contains("-af"), "{cmd}");
}

#[test]
fn a_cpu_tone_map_scales_first_maps_then_tags_the_output_bt709() {
    let cmd = line(
        h264(AudioMode::Aac),
        Some(&hevc(None)),
        None,
        pipeline(HwAccel::Software, true),
    );

    let scale = cmd.find("-vf scale=w=1920").expect(&cmd);
    let map = cmd.find("zscale=t=linear").expect(&cmd);
    let tags = cmd
        .find("-colorspace bt709 -color_primaries bt709 -color_trc bt709")
        .expect(&cmd);
    assert!(scale < map && map < tags, "{cmd}");
    assert!(cmd.contains("-pix_fmt yuv420p"), "{cmd}");
}

#[test]
fn an_sdr_re_encode_is_not_tone_mapped_or_retagged() {
    let cmd = line(
        h264(AudioMode::Aac),
        Some(&hevc(None)),
        None,
        pipeline(HwAccel::Software, false),
    );

    assert!(!cmd.contains("zscale"), "{cmd}");
    assert!(!cmd.contains("-colorspace"), "{cmd}");
}

#[test]
fn vaapi_declares_its_device_decodes_to_the_cpu_and_uploads_after_the_map() {
    let cmd = line(
        h264(AudioMode::Aac),
        Some(&hevc(None)),
        None,
        pipeline(HwAccel::Vaapi, true),
    );

    assert!(cmd.contains("-vaapi_device"), "{cmd}");
    assert!(!cmd.contains("-hwaccel"), "{cmd}");
    assert!(cmd.contains("format=yuv420p,format=nv12,hwupload"), "{cmd}");
    assert!(cmd.contains("-c:v h264_vaapi"), "{cmd}");
    assert!(cmd.find("-vaapi_device") < cmd.find(" -i "), "{cmd}");
}

#[test]
fn qsv_declares_the_derived_device_and_uploads_after_the_map() {
    let cmd = line(
        h264(AudioMode::Aac),
        Some(&hevc(None)),
        None,
        pipeline(HwAccel::Qsv, true),
    );

    assert!(cmd.contains("-init_hw_device qsv=hw@va -filter_hw_device hw"), "{cmd}");
    assert!(!cmd.contains("-hwaccel"), "{cmd}");
    assert!(cmd.contains("format=nv12,hwupload"), "{cmd}");
    assert!(cmd.contains("-c:v h264_qsv"), "{cmd}");
}

#[test]
fn nvenc_decodes_on_the_card_but_hands_system_frames_to_the_map_and_the_encoder() {
    let cmd = line(
        h264(AudioMode::Aac),
        Some(&hevc(None)),
        None,
        pipeline(HwAccel::Nvenc, true),
    );

    assert!(cmd.contains("-hwaccel cuda"), "{cmd}");
    assert!(!cmd.contains("-hwaccel_output_format"), "{cmd}");
    assert!(!cmd.contains("hwupload"), "{cmd}");
    assert!(!cmd.contains("scale_cuda"), "{cmd}");
    assert!(cmd.contains("-c:v h264_nvenc"), "{cmd}");
}

#[test]
fn videotoolbox_maps_on_the_cpu_and_encodes_the_system_frames() {
    let cmd = line(
        h264(AudioMode::Aac),
        Some(&hevc(None)),
        None,
        pipeline(HwAccel::VideoToolbox, true),
    );

    assert!(cmd.contains("-hwaccel videotoolbox"), "{cmd}");
    assert!(cmd.contains("zscale=t=linear"), "{cmd}");
    assert!(!cmd.contains("hwupload"), "{cmd}");
}

#[test]
fn without_a_tone_map_every_device_pipeline_is_the_one_it_always_was() {
    let vaapi = line(
        h264(AudioMode::Aac),
        Some(&hevc(None)),
        None,
        pipeline(HwAccel::Vaapi, false),
    );

    assert!(vaapi.contains("-hwaccel vaapi"), "{vaapi}");
    assert!(vaapi.contains("-hwaccel_output_format vaapi"), "{vaapi}");
    assert!(vaapi.contains("scale_vaapi="), "{vaapi}");
}

#[test]
fn a_seek_starts_the_decode_at_the_anchor_under_copyts() {
    let dir = PathBuf::from("/s");
    let input = MediaInput::Local(PathBuf::from("/m/film.mkv"));
    let cmd = build(&Spec {
        input: &input,
        dir: &dir,
        audio: 1,
        mode: copy(AudioMode::Copy),
        start_secs: 90.0,
        burst: true,
        pipeline: software(),
        video: None,
        track: None,
    })
    .iter()
    .map(|a| a.to_string_lossy().into_owned())
    .collect::<Vec<_>>()
    .join(" ");

    assert!(cmd.contains("-noaccurate_seek -ss 90.000"), "{cmd}");
    assert!(cmd.contains("-readrate_initial_burst 60"), "{cmd}");
    assert!(cmd.contains("-copyts -map 0:v:0 -map 0:a:1"), "{cmd}");
    assert!(cmd.ends_with("/s/index.m3u8"), "{cmd}");
}
