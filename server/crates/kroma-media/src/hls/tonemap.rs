//! Turning an HDR picture into the SDR one an 8-bit H.264 target shows.
//!
//! Without it a re-encode keeps the source's PQ or HLG curve in 8 bits and
//! comes out dark, grey and banded. The chain needs zimg (`zscale`), which not
//! every ffmpeg build has, so what this host can run is probed once, at boot.
//!
//! A tone-map is the dearest thing a re-encode can ask of a CPU: a float
//! pipeline over every pixel. So the picture is always shrunk to at most 1080p
//! BEFORE it is mapped, and [`Route`] says how much of the rest stays on the
//! device that decodes it.

use std::sync::OnceLock;
use std::time::Duration;

use kroma_module_wire::{HdrFormat, VideoStream};

use crate::input;

use super::ffmpeg::ffmpeg_output;
use super::hwaccel::HwAccel;
use super::naming::contains;

pub mod cost;
mod route;

pub use route::Routes;

const VALIDATE_TIMEOUT: Duration = Duration::from_secs(20);
const TEST_SOURCE: &str = "color=c=gray:s=320x240:r=25";

/// The tags a tone-mapped picture is written with, so a player reads the SDR
/// frames as the BT.709 they now are and not as the HDR they came from.
pub const SDR_TAGS: [&str; 6] = [
    "-colorspace",
    "bt709",
    "-color_primaries",
    "bt709",
    "-color_trc",
    "bt709",
];

const LIBPLACEBO: &str = "libplacebo=tonemapping=bt.2390:colorspace=bt709:color_primaries=bt709:color_trc=bt709:range=tv:format=yuv420p";

/// The signal a source picture is encoded in, which decides how it is
/// linearised before it is squeezed into SDR.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToneSource {
    Pq,
    Hlg,
    DolbyVision,
}

impl ToneSource {
    /// The signal `video` needs mapped, or `None` where it is SDR already or
    /// nothing reliable says what it is. Dolby Vision follows its base layer:
    /// profile 5 (no compatible base) is IPT-PQ-c2, which no plain PQ curve
    /// reads, and an SDR-compatible profile 8.2 is left alone.
    pub fn of(video: &VideoStream) -> Option<Self> {
        let transfer = video.color.as_ref().and_then(|c| c.transfer.as_deref());
        let by_transfer = || match transfer {
            Some("smpte2084") => Some(Self::Pq),
            Some("arib-std-b67") => Some(Self::Hlg),
            _ => None,
        };
        match video.hdr_format {
            Some(HdrFormat::DolbyVision) => match (
                video.dolby_vision_compatibility,
                video.dolby_vision_profile,
            ) {
                (Some(0), _) | (None, Some(5)) => Some(Self::DolbyVision),
                (Some(2), _) => None,
                (Some(4), _) => Some(Self::Hlg),
                _ => by_transfer().or(Some(Self::Pq)),
            },
            Some(HdrFormat::Hdr10 | HdrFormat::Hdr10Plus) => Some(Self::Pq),
            Some(HdrFormat::Hlg) => Some(Self::Hlg),
            None => by_transfer(),
        }
    }

    const fn transfer(self) -> &'static str {
        match self {
            Self::Hlg => "arib-std-b67",
            Self::Pq | Self::DolbyVision => "smpte2084",
        }
    }
}

/// Which of the filters a tone-map can be built from this ffmpeg actually runs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Filters {
    pub zimg: bool,
    pub placebo: bool,
}

impl Filters {
    pub const NONE: Self = Self {
        zimg: false,
        placebo: false,
    };
}

/// Where a tone-mapped re-encode runs, from the cheapest for the CPU to the
/// dearest. Decided once at boot by what the host proved it can do.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Route {
    /// The CPU decodes, scales and maps. The floor, and the only route for a
    /// stream whose reshaping data the hardware decoder does not pass on.
    Cpu,
    /// The device decodes and scales, the CPU maps a picture of at most 1080p.
    Download,
    /// Decode, scale, map and encode all stay on the device.
    Device,
}

/// One way to tone-map: zimg and the `tonemap` filter on the CPU, or libplacebo
/// where the host has a working Vulkan device, and the [`Route`] it takes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ToneMap {
    how: How,
    route: Route,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum How {
    Zscale(ToneSource),
    Placebo,
}

impl ToneMap {
    /// The way to map `source` with `filters`, on the CPU, or `None` where this
    /// host has none: a re-encode then carries the HDR curve through as it always
    /// did. Dolby Vision profile 5 has only libplacebo, which applies the
    /// stream's own reshaping; zscale cannot read it.
    pub fn resolve(source: ToneSource, filters: Filters) -> Option<Self> {
        let how = match source {
            ToneSource::DolbyVision => filters.placebo.then_some(How::Placebo),
            ToneSource::Pq | ToneSource::Hlg => filters
                .zimg
                .then_some(How::Zscale(source))
                .or_else(|| filters.placebo.then_some(How::Placebo)),
        }?;
        Some(Self {
            how,
            route: Route::Cpu,
        })
    }

    /// [`Self::resolve`], taking the route the host proved for its device.
    /// Dolby Vision stays on the CPU decoder: its reshaping data rides on the
    /// decoded frames, and a hardware decoder does not hand it over. Only
    /// PQ is mapped by a device's own filter, which reads HDR10 metadata.
    pub fn on_host(source: ToneSource, support: Support) -> Option<Self> {
        let map = Self::resolve(source, support.filters)?;
        let route = match source {
            ToneSource::DolbyVision => Route::Cpu,
            ToneSource::Pq if support.routes.device => Route::Device,
            _ if support.routes.download => Route::Download,
            _ => Route::Cpu,
        };
        Some(map.routed(route))
    }

    /// The map for `video` on this host.
    pub fn for_video(video: &VideoStream) -> Option<Self> {
        Self::on_host(ToneSource::of(video)?, support())
    }

    pub const fn route(self) -> Route {
        self.route
    }

    pub(super) const fn routed(self, route: Route) -> Self {
        Self { route, ..self }
    }

    /// The same map with every part that needs the device given up, for a
    /// picture whose size nothing knows.
    pub const fn on_the_cpu(self) -> Self {
        self.routed(Route::Cpu)
    }

    /// Whether the CPU does any of the mapping, which is what the box has to be
    /// protected for.
    pub const fn costs_cpu(self) -> bool {
        !matches!(self.route, Route::Device)
    }

    /// The filters that end in 8-bit 4:2:0 BT.709 system-memory frames. The
    /// frames are tagged first so a stream that states no colour still maps.
    pub fn chain(self) -> String {
        match self.how {
            How::Placebo => LIBPLACEBO.to_owned(),
            How::Zscale(source) => format!(
                "setparams=color_primaries=bt2020:color_trc={}:colorspace=bt2020nc:range=tv,\
                 zscale=t=linear:npl=100,format=gbrpf32le,zscale=p=bt709,\
                 tonemap=tonemap=hable:desat=0,zscale=t=bt709:m=bt709:r=tv,format=yuv420p",
                source.transfer()
            ),
        }
    }
}

/// What this host proved it can tone-map with.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Support {
    pub filters: Filters,
    pub routes: Routes,
}

impl Support {
    const NONE: Self = Self {
        filters: Filters::NONE,
        routes: Routes::NONE,
    };
}

static SUPPORT: OnceLock<Support> = OnceLock::new();

/// What the host proved at boot, or nothing before [`prime`] has finished: a
/// request that arrives first is answered conservatively, never by a probe.
pub fn support() -> Support {
    SUPPORT.get().copied().unwrap_or(Support::NONE)
}

/// The filters this ffmpeg can map with.
pub fn filters() -> Filters {
    support().filters
}

/// Settle what the host can tone-map with: its filters, then which of the
/// device routes work on `accel`. Every probe is bounded and runs here, at boot.
pub fn prime(accel: HwAccel, filters: Filters) {
    let routes = match pinned() {
        Pin::Off | Pin::Cpu => Routes::NONE,
        Pin::Auto => route::validate(accel, filters),
    };
    let _ = SUPPORT.set(Support { filters, routes });
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pin {
    Auto,
    Cpu,
    Off,
}

fn pinned() -> Pin {
    let value = std::env::var("KROMA_TONEMAP").unwrap_or_default();
    match value.trim().to_ascii_lowercase().as_str() {
        "off" | "0" | "false" => Pin::Off,
        "cpu" | "software" => Pin::Cpu,
        _ => Pin::Auto,
    }
}

/// Which of the filters this ffmpeg lists work, each made to process three
/// frames before it is believed: libplacebo is listed in every build that has it
/// and works only on a host with Vulkan.
pub fn detect_filters() -> Filters {
    if matches!(pinned(), Pin::Off) {
        return Filters::NONE;
    }
    let listed = ffmpeg_output(&["-hide_banner", "-filters"]);
    let has = |name: &str| contains(&listed, format!(" {name} ").as_bytes());
    let (zimg, placebo) = std::thread::scope(|scope| {
        let zimg = scope.spawn(|| {
            has("zscale")
                && has("tonemap")
                && validates(&ToneMap {
                    how: How::Zscale(ToneSource::Pq),
                    route: Route::Cpu,
                })
        });
        let placebo = has("libplacebo")
            && validates(&ToneMap {
                how: How::Placebo,
                route: Route::Cpu,
            });
        (zimg.join().unwrap_or(false), placebo)
    });
    Filters { zimg, placebo }
}

fn validates(map: &ToneMap) -> bool {
    let mut cmd = std::process::Command::new("ffmpeg");
    cmd.args(["-v", "error", "-nostdin"])
        .args(input::generated(TEST_SOURCE))
        .args(["-frames:v", "3", "-vf"])
        .arg(format!("format=yuv420p10le,{}", map.chain()))
        .args(["-f", "null", "-"]);
    crate::ffmpeg_run::run_capturing(cmd, VALIDATE_TIMEOUT).is_ok()
}

#[cfg(test)]
mod tests;
