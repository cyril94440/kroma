use super::*;

fn encode(accel: HwAccel, route: Route) -> String {
    line(h264(AudioMode::Aac), Some(&hevc(None)), None, routed(accel, route))
}

#[test]
fn a_cpu_tone_map_scales_to_1080p_first_maps_then_tags_the_output_bt709() {
    let cmd = line(
        h264(AudioMode::Aac),
        Some(&hevc(None)),
        None,
        pipeline(HwAccel::Software, true),
    );

    let scale = cmd.find("-vf scale=w=1920:h=1080,setparams").expect(&cmd);
    let map = cmd.find("zscale=t=linear").expect(&cmd);
    let tags = cmd
        .find("-colorspace bt709 -color_primaries bt709 -color_trc bt709")
        .expect(&cmd);
    assert!(scale < map && map < tags, "{cmd}");
    assert!(cmd.contains("-pix_fmt yuv420p"), "{cmd}");
}

#[test]
fn a_tone_map_never_runs_above_1080p_whatever_the_box_asked_for() {
    let full_size = Pipeline {
        frame: Some((7680, 4320)),
        ..pipeline(HwAccel::Software, true)
    };

    let filter = full_size.video_filter(None).unwrap();

    assert!(filter.starts_with("scale=w=1920:h=1080,setparams"), "{filter}");
}

#[test]
fn a_smaller_box_than_1080p_is_kept() {
    let filter = pipeline(HwAccel::Software, true)
        .video_filter(Some((1280, 720)))
        .unwrap();

    assert!(filter.starts_with("scale=w=1280:h=720,setparams"), "{filter}");
}

#[test]
fn a_scope_frame_is_shrunk_by_its_width_and_never_stretched() {
    let scope = Pipeline {
        frame: Some((3840, 1604)),
        ..pipeline(HwAccel::Software, true)
    };

    let filter = scope.video_filter(None).unwrap();

    assert!(filter.starts_with("scale=w=1920:h=802,"), "{filter}");
}

#[test]
fn a_picture_already_inside_1080p_is_mapped_without_being_resized() {
    let hd = Pipeline {
        frame: Some((1920, 1080)),
        ..pipeline(HwAccel::Software, true)
    };

    let filter = hd.video_filter(None).unwrap();

    assert!(filter.starts_with("setparams="), "{filter}");
    assert!(!filter.contains("scale=w"), "{filter}");
}

#[test]
fn a_source_nobody_measured_is_fitted_into_the_box_on_the_cpu_whatever_route_was_proved() {
    let unknown = Pipeline {
        frame: None,
        ..routed(HwAccel::Vaapi, Route::Device)
    };

    let filter = unknown.video_filter(None).unwrap();
    let input = unknown.input_args().join(" ");

    assert!(
        filter.starts_with(
            "scale=w=1920:h=1080:force_original_aspect_ratio=decrease:force_divisible_by=2,setparams"
        ),
        "{filter}"
    );
    assert!(filter.ends_with("format=nv12,hwupload"), "{filter}");
    assert!(!input.contains("-hwaccel"), "{input}");
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
fn vaapi_on_the_cpu_route_decodes_on_the_cpu_and_uploads_after_the_map() {
    let cmd = encode(HwAccel::Vaapi, Route::Cpu);

    assert!(cmd.contains("-vaapi_device"), "{cmd}");
    assert!(!cmd.contains("-hwaccel"), "{cmd}");
    assert!(cmd.contains("format=yuv420p,format=nv12,hwupload"), "{cmd}");
    assert!(cmd.contains("-c:v h264_vaapi"), "{cmd}");
    assert!(cmd.find("-vaapi_device") < cmd.find(" -i "), "{cmd}");
}

#[test]
fn vaapi_keeps_its_hardware_decode_and_scale_and_downloads_a_1080p_picture_to_map() {
    let cmd = encode(HwAccel::Vaapi, Route::Download);

    assert!(cmd.contains("-hwaccel vaapi"), "{cmd}");
    assert!(cmd.contains("-hwaccel_output_format vaapi"), "{cmd}");
    assert!(cmd.find("-hwaccel vaapi") < cmd.find(" -i "), "{cmd}");
    assert!(
        cmd.contains("-vf scale_vaapi=w=1920:h=1080,hwdownload,format=p010le,setparams="),
        "{cmd}"
    );
    assert!(cmd.contains("format=yuv420p,format=nv12,hwupload"), "{cmd}");
    assert!(cmd.contains("-vaapi_device"), "{cmd}");
    assert!(cmd.contains("-c:v h264_vaapi"), "{cmd}");
}

#[test]
fn vaapi_can_map_on_the_device_and_never_touches_the_cpu() {
    let cmd = encode(HwAccel::Vaapi, Route::Device);

    assert!(
        cmd.contains("-vf scale_vaapi=w=1920:h=1080,tonemap_vaapi=format=nv12:t=bt709:m=bt709:p=bt709"),
        "{cmd}"
    );
    assert!(cmd.contains("-hwaccel_output_format vaapi"), "{cmd}");
    for cpu in ["hwdownload", "zscale", "hwupload", "-vaapi_device"] {
        assert!(!cmd.contains(cpu), "{cpu} in {cmd}");
    }
}

#[test]
fn qsv_declares_the_derived_device_and_uploads_after_the_map_on_the_cpu_route() {
    let cmd = encode(HwAccel::Qsv, Route::Cpu);

    assert!(
        cmd.contains("-init_hw_device qsv=hw@va -filter_hw_device hw"),
        "{cmd}"
    );
    assert!(!cmd.contains("-hwaccel"), "{cmd}");
    assert!(cmd.contains("format=nv12,hwupload"), "{cmd}");
    assert!(cmd.contains("-c:v h264_qsv"), "{cmd}");
}

#[test]
fn qsv_keeps_its_hardware_decode_and_scale_and_downloads_a_1080p_picture_to_map() {
    let cmd = encode(HwAccel::Qsv, Route::Download);

    assert!(cmd.contains("-hwaccel qsv -hwaccel_output_format qsv"), "{cmd}");
    assert!(cmd.contains("-filter_hw_device hw"), "{cmd}");
    assert!(
        cmd.contains("-vf scale_qsv=w=1920:h=1080,hwdownload,format=p010le,setparams="),
        "{cmd}"
    );
    assert!(cmd.contains("format=nv12,hwupload"), "{cmd}");
}

#[test]
fn qsv_can_scale_and_map_in_one_vpp_pass() {
    let cmd = encode(HwAccel::Qsv, Route::Device);

    assert!(
        cmd.contains(
            "-vf vpp_qsv=w=1920:h=1080:tonemap=1:format=nv12:out_color_matrix=bt709:\
             out_color_primaries=bt709:out_color_transfer=bt709"
        ),
        "{cmd}"
    );
    for cpu in ["hwdownload", "zscale", "hwupload", "scale_qsv"] {
        assert!(!cmd.contains(cpu), "{cpu} in {cmd}");
    }
}

#[test]
fn nvenc_decodes_and_scales_on_the_card_and_downloads_a_1080p_picture_to_map() {
    let cmd = encode(HwAccel::Nvenc, Route::Download);

    assert!(cmd.contains("-hwaccel cuda -hwaccel_output_format cuda"), "{cmd}");
    assert!(
        cmd.contains("-vf scale_cuda=w=1920:h=1080,hwdownload,format=p010le,setparams="),
        "{cmd}"
    );
    assert!(!cmd.contains("hwupload"), "{cmd}");
    assert!(cmd.contains("-c:v h264_nvenc"), "{cmd}");
}

#[test]
fn nvenc_on_the_cpu_route_still_decodes_on_the_card_and_hands_system_frames_over() {
    let cmd = encode(HwAccel::Nvenc, Route::Cpu);

    assert!(cmd.contains("-hwaccel cuda"), "{cmd}");
    assert!(!cmd.contains("-hwaccel_output_format"), "{cmd}");
    assert!(cmd.contains("-vf scale=w=1920:h=1080,setparams"), "{cmd}");
    assert!(!cmd.contains("scale_cuda"), "{cmd}");
}

#[test]
fn nvenc_can_map_on_the_device() {
    let cmd = encode(HwAccel::Nvenc, Route::Device);

    assert!(
        cmd.contains("-vf scale_cuda=w=1920:h=1080,tonemap_cuda=format=yuv420p:p=bt709:t=bt709:m=bt709:tonemap=bt2390"),
        "{cmd}"
    );
    assert!(!cmd.contains("hwdownload"), "{cmd}");
}

#[test]
fn videotoolbox_decodes_on_the_block_and_maps_a_1080p_picture_on_the_cpu() {
    for route in [Route::Cpu, Route::Download] {
        let cmd = encode(HwAccel::VideoToolbox, route);

        assert!(cmd.contains("-hwaccel videotoolbox"), "{cmd}");
        assert!(cmd.contains("-vf scale=w=1920:h=1080,setparams"), "{cmd}");
        assert!(!cmd.contains("hwupload") && !cmd.contains("hwdownload"), "{cmd}");
    }
}

#[test]
fn a_device_route_is_ignored_on_a_host_with_no_device() {
    let cmd = encode(HwAccel::Software, Route::Device);

    assert!(cmd.contains("-vf scale=w=1920:h=1080,setparams"), "{cmd}");
    assert!(!cmd.contains("-hwaccel"), "{cmd}");
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
