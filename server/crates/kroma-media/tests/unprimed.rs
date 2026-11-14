//! A request that arrives before the boot probe has finished is answered with
//! the conservative reading and never waits on a child process. Nothing in this
//! file primes anything, so it holds for the whole process.

use std::time::{Duration, Instant};

use kroma_media::capabilities;
use kroma_media::hls::{tonemap_filters, Filters, HlsEngine, ToneMap};
use kroma_module_wire::{HdrFormat, VideoStream};

const INSTANT: Duration = Duration::from_millis(100);

fn hdr10() -> VideoStream {
    VideoStream {
        codec: "hevc".into(),
        width: Some(3840),
        height: Some(2160),
        hdr: true,
        bit_depth: Some(10),
        hdr_format: Some(HdrFormat::Hdr10),
        dolby_vision_profile: None,
        dolby_vision_level: None,
        dolby_vision_compatibility: None,
        dolby_vision_enhancement_layer: false,
        color: None,
    }
}

#[test]
fn before_the_probe_nothing_is_tone_mapped_and_nothing_waits() {
    let started = Instant::now();

    let map = ToneMap::for_video(&hdr10());
    let filters = tonemap_filters();

    assert!(map.is_none());
    assert_eq!(
        filters,
        Filters {
            zimg: false,
            placebo: false
        }
    );
    assert!(started.elapsed() < INSTANT, "{:?}", started.elapsed());
}

#[test]
fn before_the_probe_the_host_has_no_device_and_says_why() {
    let started = Instant::now();

    let hardware = HlsEngine::hardware();

    assert_eq!(hardware.accel.label(), "software");
    assert!(
        hardware.reason.contains("not finished"),
        "{}",
        hardware.reason
    );
    assert!(started.elapsed() < INSTANT, "{:?}", started.elapsed());
}

#[test]
fn before_the_probe_the_build_is_assumed_to_have_no_network_input_and_no_surround_encoders() {
    let started = Instant::now();

    let network = capabilities::network_input();
    let audio = capabilities::audio_targets();

    assert!(!network);
    assert!(!audio.ac3 && !audio.eac3);
    assert!(started.elapsed() < INSTANT, "{:?}", started.elapsed());
}
