use super::*;

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
