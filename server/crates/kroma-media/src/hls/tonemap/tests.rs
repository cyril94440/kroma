use kroma_module_wire::ColorInfo;

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
