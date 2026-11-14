//! What a tone-mapped frame costs, so a host is not offered a rung it cannot hold.
//!
//! Nothing here watches a running session; the guard is to choose the rung well
//! before ffmpeg starts. The figures are milliseconds per frame on four cores,
//! measured on a 3840x2160 10-bit HEVC source tone-mapped with zscale to
//! BT.709 and encoded with libx264 veryfast, over 288 frames so that process
//! start-up does not count: the decode alone took 13 ms a frame, the scale and
//! tone-map 6.4 ms per output megapixel and the encode 4.9.

use super::super::mode::Rung;
use super::Route;

const MEASURED_CORES: f64 = 4.0;
const DECODE_MS_PER_SOURCE_MPIX: f64 = 1.6;
const MAP_MS_PER_TARGET_MPIX: f64 = 6.4;
const ENCODE_MS_PER_TARGET_MPIX: f64 = 4.9;
const ASSUMED_FPS: f64 = 30.0;
const SMALLEST: Rung = Rung::P720;

fn mpix((w, h): (u32, u32)) -> f64 {
    f64::from(w) * f64::from(h) / 1_000_000.0
}

pub(in crate::hls) fn frame_ms(
    source: (u32, u32),
    target: (u32, u32),
    route: Route,
    encode_on_the_cpu: bool,
    cores: usize,
) -> f64 {
    let decode = if route == Route::Cpu {
        DECODE_MS_PER_SOURCE_MPIX * mpix(source)
    } else {
        0.0
    };
    let encode = if encode_on_the_cpu {
        ENCODE_MS_PER_TARGET_MPIX * mpix(target)
    } else {
        0.0
    };
    (decode + MAP_MS_PER_TARGET_MPIX * mpix(target) + encode) * MEASURED_CORES / cores.max(1) as f64
}

/// The largest rung a tone-mapped `source` can be served at in real time on this
/// host, or the smallest rung where none can: a picture that arrives slowly is
/// still better than none, and below it there is nothing left to give. A device
/// route is not the CPU's problem, and a source already inside 1080p is not being
/// shrunk, so both keep the 1080p box.
pub(in crate::hls) fn affordable(
    source: (u32, u32),
    route: Route,
    encode_on_the_cpu: bool,
    cores: usize,
) -> Rung {
    let largest = Rung::P1080.box_size();
    if route == Route::Device || (source.0 <= largest.0 && source.1 <= largest.1) {
        return Rung::P1080;
    }
    let budget = 1000.0 / ASSUMED_FPS;
    [Rung::P1080, Rung::P720]
        .into_iter()
        .find(|rung| frame_ms(source, rung.box_size(), route, encode_on_the_cpu, cores) <= budget)
        .unwrap_or(SMALLEST)
}

#[cfg(test)]
mod tests {
    use super::*;

    const UHD: (u32, u32) = (3840, 2160);
    const HD: (u32, u32) = (1920, 1080);

    #[test]
    fn the_model_reproduces_what_four_cores_measured() {
        let at_1080 = frame_ms(UHD, HD, Route::Cpu, true, 4);
        let at_720 = frame_ms(UHD, (1280, 720), Route::Cpu, true, 4);

        assert!((at_1080 - 37.0).abs() < 3.0, "{at_1080}");
        assert!((at_720 - 22.9).abs() < 3.0, "{at_720}");
    }

    #[test]
    fn a_four_core_host_does_not_tone_map_4k_at_1080p_in_software() {
        assert_eq!(affordable(UHD, Route::Cpu, true, 4), Rung::P720);
    }

    #[test]
    fn a_host_with_headroom_keeps_1080p() {
        assert_eq!(affordable(UHD, Route::Cpu, true, 16), Rung::P1080);
    }

    #[test]
    fn a_device_route_is_not_the_cpus_problem() {
        assert_eq!(affordable(UHD, Route::Device, false, 2), Rung::P1080);
    }

    #[test]
    fn keeping_the_decode_on_the_device_is_what_buys_the_headroom_back() {
        let on_cpu = frame_ms(UHD, HD, Route::Cpu, false, 4);
        let on_device = frame_ms(UHD, HD, Route::Download, false, 4);

        assert!(on_device < on_cpu);
    }

    #[test]
    fn a_source_inside_1080p_is_not_shrunk_for_the_tone_map() {
        assert_eq!(affordable(HD, Route::Cpu, true, 2), Rung::P1080);
    }
}
