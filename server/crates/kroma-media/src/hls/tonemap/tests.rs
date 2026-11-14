use kroma_module_wire::{ColorInfo, HdrFormat};

use super::*;

const BOTH: Filters = Filters {
    zimg: true,
    placebo: true,
};
const NEITHER: Filters = Filters {
    zimg: false,
    placebo: false,
};
const ZIMG: Filters = Filters {
    zimg: true,
    placebo: false,
};
const PLACEBO: Filters = Filters {
    zimg: false,
    placebo: true,
};

fn video(format: Option<HdrFormat>, transfer: Option<&str>) -> VideoStream {
    VideoStream {
        codec: "hevc".into(),
        width: Some(3840),
        height: Some(2160),
        hdr: format.is_some(),
        bit_depth: Some(10),
        hdr_format: format,
        dolby_vision_profile: None,
        dolby_vision_level: None,
        dolby_vision_compatibility: None,
        dolby_vision_enhancement_layer: false,
        color: transfer.map(|t| ColorInfo {
            transfer: Some(t.into()),
            ..ColorInfo::default()
        }),
    }
}

fn dolby(profile: u32, compat: Option<u8>, transfer: Option<&str>) -> VideoStream {
    VideoStream {
        dolby_vision_profile: Some(profile),
        dolby_vision_compatibility: compat,
        ..video(Some(HdrFormat::DolbyVision), transfer)
    }
}

#[test]
fn an_sdr_picture_has_nothing_to_map() {
    assert_eq!(ToneSource::of(&video(None, Some("bt709"))), None);
    assert_eq!(ToneSource::of(&video(None, None)), None);
}

#[test]
fn hdr10_hdr10_plus_and_hlg_map_from_their_own_curve() {
    assert_eq!(
        ToneSource::of(&video(Some(HdrFormat::Hdr10), None)),
        Some(ToneSource::Pq)
    );
    assert_eq!(
        ToneSource::of(&video(Some(HdrFormat::Hdr10Plus), Some("smpte2084"))),
        Some(ToneSource::Pq)
    );
    assert_eq!(
        ToneSource::of(&video(Some(HdrFormat::Hlg), Some("arib-std-b67"))),
        Some(ToneSource::Hlg)
    );
}

#[test]
fn a_row_that_only_ever_recorded_hdr_is_mapped_by_the_transfer_it_states() {
    assert_eq!(
        ToneSource::of(&video(None, Some("smpte2084"))),
        Some(ToneSource::Pq)
    );
    assert_eq!(
        ToneSource::of(&video(None, Some("arib-std-b67"))),
        Some(ToneSource::Hlg)
    );
}

#[test]
fn dolby_vision_follows_the_base_layer_it_declares() {
    assert_eq!(
        ToneSource::of(&dolby(5, Some(0), None)),
        Some(ToneSource::DolbyVision)
    );
    assert_eq!(
        ToneSource::of(&dolby(5, None, None)),
        Some(ToneSource::DolbyVision)
    );
    assert_eq!(
        ToneSource::of(&dolby(8, Some(1), Some("smpte2084"))),
        Some(ToneSource::Pq)
    );
    assert_eq!(
        ToneSource::of(&dolby(7, Some(6), Some("smpte2084"))),
        Some(ToneSource::Pq)
    );
    assert_eq!(
        ToneSource::of(&dolby(8, Some(4), Some("arib-std-b67"))),
        Some(ToneSource::Hlg)
    );
    assert_eq!(ToneSource::of(&dolby(8, Some(2), Some("bt709"))), None);
}

#[test]
fn the_cpu_chain_linearises_tone_maps_and_lands_on_bt709_8_bit() {
    let chain = ToneMap::resolve(ToneSource::Pq, ZIMG).unwrap().chain();

    let order = [
        "setparams=color_primaries=bt2020:color_trc=smpte2084",
        "zscale=t=linear",
        "format=gbrpf32le",
        "zscale=p=bt709",
        "tonemap=tonemap=hable",
        "zscale=t=bt709:m=bt709:r=tv",
        "format=yuv420p",
    ];
    let mut from = 0;
    for step in order {
        let at = chain[from..]
            .find(step)
            .unwrap_or_else(|| panic!("{step} missing or out of order in {chain}"));
        from += at + step.len();
    }
}

#[test]
fn hlg_is_linearised_by_its_own_curve() {
    let chain = ToneMap::resolve(ToneSource::Hlg, ZIMG).unwrap().chain();

    assert!(chain.contains("color_trc=arib-std-b67"), "{chain}");
    assert!(!chain.contains("smpte2084"), "{chain}");
}

#[test]
fn dolby_vision_profile_5_needs_libplacebo_and_has_no_other_way() {
    assert!(ToneMap::resolve(ToneSource::DolbyVision, ZIMG).is_none());
    assert!(ToneMap::resolve(ToneSource::DolbyVision, NEITHER).is_none());
    let chain = ToneMap::resolve(ToneSource::DolbyVision, BOTH)
        .unwrap()
        .chain();
    assert!(chain.starts_with("libplacebo="), "{chain}");
    assert!(chain.ends_with("format=yuv420p"), "{chain}");
}

#[test]
fn a_host_without_zimg_falls_back_on_libplacebo_and_without_either_on_nothing() {
    let placebo = ToneMap::resolve(ToneSource::Pq, PLACEBO).unwrap().chain();

    assert!(placebo.starts_with("libplacebo="), "{placebo}");
    assert!(ToneMap::resolve(ToneSource::Pq, NEITHER).is_none());
    assert!(ToneMap::resolve(ToneSource::Hlg, NEITHER).is_none());
}

#[test]
fn zimg_is_preferred_over_libplacebo_for_pq_and_hlg() {
    let chain = ToneMap::resolve(ToneSource::Pq, BOTH).unwrap().chain();

    assert!(chain.contains("zscale"), "{chain}");
}

#[test]
fn the_output_is_tagged_bt709_on_all_three_axes() {
    assert_eq!(
        SDR_TAGS,
        [
            "-colorspace",
            "bt709",
            "-color_primaries",
            "bt709",
            "-color_trc",
            "bt709"
        ]
    );
}

fn run_chain(map: &ToneMap, graph: &str) -> bool {
    let mut cmd = std::process::Command::new("ffmpeg");
    cmd.args(["-v", "error", "-nostdin"])
        .args(input::generated(graph))
        .args(["-frames:v", "3", "-vf"])
        .arg(map.chain())
        .args(["-f", "null", "-"]);
    cmd.status().is_ok_and(|s| s.success())
}

#[test]
fn the_cpu_chain_runs_on_a_pq_source_that_states_no_colour_at_all() {
    if !filters().zimg {
        return;
    }
    let map = ToneMap::resolve(ToneSource::Pq, ZIMG).unwrap();

    assert!(run_chain(
        &map,
        "testsrc2=size=320x180:rate=25,format=yuv420p10le"
    ));
}

fn proved(device: bool, download: bool) -> Support {
    Support {
        filters: BOTH,
        routes: Routes { device, download },
    }
}

#[test]
fn a_host_that_proved_nothing_maps_on_the_cpu_route() {
    for source in [ToneSource::Pq, ToneSource::Hlg, ToneSource::DolbyVision] {
        let map = ToneMap::on_host(source, proved(false, false)).unwrap();

        assert_eq!(map.route(), Route::Cpu, "{source:?}");
        assert!(map.costs_cpu());
    }
}

#[test]
fn a_proved_device_filter_maps_hdr10_and_keeps_the_cpu_out_of_it() {
    let map = ToneMap::on_host(ToneSource::Pq, proved(true, true)).unwrap();

    assert_eq!(map.route(), Route::Device);
    assert!(!map.costs_cpu());
}

#[test]
fn hlg_never_goes_to_a_device_filter_that_reads_hdr10_metadata() {
    let map = ToneMap::on_host(ToneSource::Hlg, proved(true, true)).unwrap();

    assert_eq!(map.route(), Route::Download);
    assert!(map.costs_cpu());
}

#[test]
fn dolby_vision_profile_5_stays_on_the_cpu_decoder_that_hands_over_its_reshaping() {
    let map = ToneMap::on_host(ToneSource::DolbyVision, proved(true, true)).unwrap();

    assert_eq!(map.route(), Route::Cpu);
}

#[test]
fn a_route_is_only_offered_on_top_of_a_working_cpu_chain() {
    let none = Support {
        filters: NEITHER,
        routes: Routes {
            device: true,
            download: true,
        },
    };

    assert!(ToneMap::on_host(ToneSource::Pq, none).is_none());
}

#[test]
fn without_a_frame_size_the_device_routes_give_way_to_the_cpu() {
    let map = ToneMap::on_host(ToneSource::Pq, proved(true, true)).unwrap();

    assert_eq!(map.on_the_cpu().route(), Route::Cpu);
}

#[test]
fn before_the_boot_probe_has_finished_nothing_can_be_tone_mapped() {
    assert!(ToneMap::on_host(ToneSource::Pq, Support::NONE).is_none());
}

#[test]
fn a_host_with_no_device_proves_no_device_route() {
    assert_eq!(route::validate(HwAccel::Software, BOTH), Routes::NONE);
}

#[test]
fn a_host_with_no_cpu_chain_proves_no_device_route_either() {
    assert_eq!(route::validate(HwAccel::Vaapi, NEITHER), Routes::NONE);
}

#[test]
fn videotoolbox_keeps_its_hardware_decode_with_nothing_to_validate() {
    assert_eq!(
        route::validate(HwAccel::VideoToolbox, ZIMG),
        Routes {
            device: false,
            download: true
        }
    );
}

#[test]
fn a_device_that_cannot_run_the_vectors_proves_neither_and_the_cpu_route_stands() {
    // No device here: every vector fails inside ffmpeg and is not offered.
    if std::path::Path::new("/dev/dri").exists() {
        return;
    }
    assert_eq!(route::validate(HwAccel::Vaapi, ZIMG), Routes::NONE);
}

#[test]
fn the_clip_a_route_is_proved_on_is_a_small_hdr10_stream_with_its_mastering_metadata() {
    let Some(clip) = route::Clip::make() else {
        return;
    };
    let ffprobe = |args: &[&str]| {
        let mut cmd = std::process::Command::new("ffprobe");
        input::without_proxies(&mut cmd);
        let out = cmd
            .args(["-v", "error", "-select_streams", "v:0"])
            .args(args)
            .args(["-of", "csv=p=0"])
            .args(input::media(&input::MediaInput::Local(clip.path().to_path_buf())))
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    let stream = ffprobe(&["-show_entries", "stream=width,color_transfer,pix_fmt"]);
    let frame = ffprobe(&["-read_intervals", "%+#1", "-show_entries", "frame_side_data_list"]);

    assert_eq!(stream.trim(), "640,yuv420p10le,smpte2084");
    assert!(frame.contains("Mastering display"), "{frame}");
    let path = clip.path().to_path_buf();
    drop(clip);
    assert!(!path.exists(), "the clip is removed once the proof is done");
}
