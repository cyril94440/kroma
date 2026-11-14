use super::*;

fn video_of(stream_json: &str) -> VideoStream {
    let raw: FfprobeOutput =
        serde_json::from_str(&format!(r#"{{"streams":[{stream_json}]}}"#)).unwrap();
    build_result(raw)
        .video
        .expect("the fixture describes one video stream")
}

#[test]
fn a_dolby_vision_record_outranks_the_transfer_and_keeps_its_profile() {
    let video = video_of(
        r#"{"codec_type":"video","codec_name":"hevc","pix_fmt":"yuv420p10le",
            "color_transfer":"smpte2084","color_primaries":"bt2020","color_space":"bt2020nc",
            "side_data_list":[{"side_data_type":"DOVI configuration record","dv_profile":5}]}"#,
    );

    assert_eq!(video.hdr_format, Some(HdrFormat::DolbyVision));
    assert_eq!(video.dolby_vision_profile, Some(5));
    assert!(video.hdr);
}

#[test]
fn pq_reads_as_hdr10_and_the_arib_transfer_as_hlg() {
    let pq =
        video_of(r#"{"codec_type":"video","codec_name":"hevc","color_transfer":"smpte2084"}"#);
    let hlg = video_of(
        r#"{"codec_type":"video","codec_name":"hevc","color_transfer":"arib-std-b67"}"#,
    );

    assert_eq!(pq.hdr_format, Some(HdrFormat::Hdr10));
    assert_eq!(hlg.hdr_format, Some(HdrFormat::Hlg));
    assert!(pq.dolby_vision_profile.is_none());
}

#[test]
fn ten_bit_bt2020_with_no_transfer_stated_still_reads_as_hdr10() {
    let video = video_of(
        r#"{"codec_type":"video","codec_name":"hevc","pix_fmt":"yuv420p10le",
            "color_primaries":"bt2020"}"#,
    );

    assert_eq!(video.hdr_format, Some(HdrFormat::Hdr10));
    assert_eq!(video.bit_depth, Some(10));
}

#[test]
fn an_sdr_stream_names_no_variant_and_its_colour_still_travels() {
    let video = video_of(
        r#"{"codec_type":"video","codec_name":"h264","pix_fmt":"yuv420p",
            "color_transfer":"bt709","color_primaries":"bt709","color_space":"bt709"}"#,
    );

    assert!(video.hdr_format.is_none());
    assert!(!video.hdr);
    let color = video
        .color
        .expect("bt709 is colour metadata like any other");
    assert_eq!(color.primaries.as_deref(), Some("bt709"));
    assert_eq!(color.transfer.as_deref(), Some("bt709"));
    assert_eq!(color.matrix.as_deref(), Some("bt709"));
}

#[test]
fn a_stream_that_states_no_colour_at_all_carries_none() {
    let video = video_of(r#"{"codec_type":"video","codec_name":"h264"}"#);

    assert!(video.color.is_none());
    assert!(video.hdr_format.is_none());
}

#[test]
fn a_subtitle_track_keeps_its_title_and_forced_flag() {
    let raw: FfprobeOutput = serde_json::from_str(
        r#"{"streams":[
            {"codec_type":"subtitle","codec_name":"subrip",
             "tags":{"language":"fre","title":"Forcés"},"disposition":{"default":0,"forced":1}},
            {"codec_type":"subtitle","codec_name":"subrip","tags":{"language":"fre"}}]}"#,
    )
    .unwrap();

    let subs = build_result(raw).subtitles;

    assert_eq!(subs[0].title.as_deref(), Some("Forcés"));
    assert!(subs[0].forced);
    assert!(subs[1].title.is_none());
    assert!(!subs[1].forced);
}

fn dovi(profile: u32, level: u32, el: u32, compat: u8) -> String {
    format!(
        r#"{{"codec_type":"video","codec_name":"hevc","pix_fmt":"yuv420p10le",
            "color_transfer":"smpte2084","color_primaries":"bt2020","color_space":"bt2020nc",
            "side_data_list":[{{"side_data_type":"DOVI configuration record",
            "dv_version_major":1,"dv_version_minor":0,"dv_profile":{profile},"dv_level":{level},
            "rpu_present_flag":1,"el_present_flag":{el},"bl_present_flag":1,
            "dv_bl_signal_compatibility_id":{compat}}}]}}"#
    )
}

#[test]
fn the_dovi_record_of_a_profile_5_file_has_no_compatible_base_layer() {
    let video = video_of(&dovi(5, 4, 0, 0));

    assert_eq!(video.dolby_vision_profile, Some(5));
    assert_eq!(video.dolby_vision_level, Some(4));
    assert_eq!(video.dolby_vision_compatibility, Some(0));
    assert!(!video.dolby_vision_enhancement_layer);
}

#[test]
fn a_profile_8_file_names_the_base_layer_it_falls_back_on() {
    for (compat, name) in [(1u8, "hdr10"), (2, "sdr"), (4, "hlg")] {
        let video = video_of(&dovi(8, 6, 0, compat));

        assert_eq!(video.dolby_vision_compatibility, Some(compat), "{name}");
        assert_eq!(video.hdr_format, Some(HdrFormat::DolbyVision), "{name}");
    }
}

#[test]
fn a_single_track_profile_7_file_flags_its_enhancement_layer() {
    let video = video_of(&dovi(7, 6, 1, 6));

    assert!(video.dolby_vision_enhancement_layer);
    assert_eq!(video.dolby_vision_compatibility, Some(6));
}

#[test]
fn a_stream_without_a_dovi_record_carries_no_dolby_vision_detail() {
    let video =
        video_of(r#"{"codec_type":"video","codec_name":"hevc","color_transfer":"smpte2084"}"#);

    assert_eq!(video.dolby_vision_level, None);
    assert_eq!(video.dolby_vision_compatibility, None);
    assert!(!video.dolby_vision_enhancement_layer);
}

#[test]
fn ten_bit_bt2020_that_states_an_sdr_transfer_is_not_hdr() {
    for transfer in ["bt709", "bt2020-10", "bt2020-12"] {
        let video = video_of(&format!(
            r#"{{"codec_type":"video","codec_name":"hevc","pix_fmt":"yuv420p10le",
                "color_primaries":"bt2020","color_transfer":"{transfer}"}}"#
        ));

        assert!(video.hdr_format.is_none(), "{transfer}");
        assert!(!video.hdr, "{transfer}");
    }
}

#[test]
fn a_transfer_ffprobe_calls_unknown_is_as_good_as_none_stated() {
    let video = video_of(
        r#"{"codec_type":"video","codec_name":"hevc","pix_fmt":"yuv420p10le",
            "color_primaries":"bt2020","color_transfer":"unknown"}"#,
    );

    assert_eq!(video.hdr_format, Some(HdrFormat::Hdr10));
}

fn audio_of(stream_json: &str) -> AudioStream {
    let raw: FfprobeOutput =
        serde_json::from_str(&format!(r#"{{"streams":[{stream_json}]}}"#)).unwrap();
    build_result(raw)
        .audio
        .expect("the fixture describes one audio stream")
}

#[test]
fn a_truehd_track_named_atmos_is_spatial_and_keeps_its_profile_and_layout() {
    let audio = audio_of(
        r#"{"codec_type":"audio","codec_name":"truehd","profile":"Dolby TrueHD + Dolby Atmos",
            "channels":8,"channel_layout":"7.1"}"#,
    );

    assert_eq!(audio.codec, "truehd");
    assert_eq!(audio.spatial, Some(SpatialAudio::Atmos));
    assert_eq!(audio.profile.as_deref(), Some("Dolby TrueHD + Dolby Atmos"));
    assert_eq!(audio.layout.as_deref(), Some("7.1"));
}

#[test]
fn every_profile_string_ffprobe_prints_maps_to_one_spatial_flag() {
    for (profile, spatial) in [
        ("Dolby Digital Plus + Dolby Atmos", Some(SpatialAudio::Atmos)),
        ("Dolby TrueHD + Dolby Atmos", Some(SpatialAudio::Atmos)),
        ("DTS-HD MA + DTS:X", Some(SpatialAudio::Dtsx)),
        ("DTS-HD MA + DTS:X IMAX", Some(SpatialAudio::Dtsx)),
        ("DTS-HD MA", None),
        ("DTS-HD HRA", None),
        ("DTS", None),
        ("DTS-ES", None),
        ("DTS Express", None),
        ("LC", None),
        ("", None),
    ] {
        assert_eq!(spatial_of(profile), spatial, "{profile}");
    }
}

#[test]
fn dts_hd_master_audio_stays_dts_but_shows_in_the_profile() {
    let audio = audio_of(
        r#"{"codec_type":"audio","codec_name":"dts","profile":"DTS-HD MA",
            "channels":8,"channel_layout":"7.1"}"#,
    );

    assert_eq!(audio.codec, "dts");
    assert_eq!(audio.profile.as_deref(), Some("DTS-HD MA"));
    assert!(audio.spatial.is_none());
}

#[test]
fn a_track_with_no_profile_or_layout_carries_neither() {
    let audio = audio_of(r#"{"codec_type":"audio","codec_name":"ac3","channels":6,"channel_layout":""}"#);

    assert!(audio.profile.is_none());
    assert!(audio.layout.is_none());
    assert!(audio.spatial.is_none());
}

#[test]
fn the_stream_index_of_the_video_skips_a_leading_cover_art_stream() {
    let raw: FfprobeOutput = serde_json::from_str(
        r#"{"streams":[
            {"index":0,"codec_type":"video","codec_name":"mjpeg","width":600,"height":600},
            {"index":1,"codec_type":"audio","codec_name":"aac"},
            {"index":2,"codec_type":"video","codec_name":"hevc","width":3840,"height":2160}]}"#,
    )
    .unwrap();

    assert_eq!(video_index(&raw), Some(2));
}
