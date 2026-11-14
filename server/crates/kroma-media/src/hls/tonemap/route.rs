//! Which device routes a tone-map can take on this host, proved at boot.
//!
//! No device argument vector here can be trusted from a manual: a filter that is
//! listed proves nothing about a driver. Each route is run on a tiny HDR10 clip
//! through the exact `-hwaccel` flags and `-vf` a session would use, decode
//! included, and is believed only if three frames come out. A route that fails
//! is simply not offered and the next one down is, so a wrong vector costs speed,
//! never playback.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::input::{self, MediaInput};

use super::super::ffmpeg::ffmpeg_output;
use super::super::hwaccel::{HwAccel, Pipeline};
use super::super::naming::contains;
use super::super::software::Effort;
use super::{Filters, Route, ToneMap, ToneSource};

const ROUTE_TIMEOUT: Duration = Duration::from_secs(15);
const CLIP_SIZE: (u32, u32) = (640, 360);
const TARGET: (u32, u32) = (320, 180);
const CLIP_SOURCE: &str = "testsrc2=s=640x360:r=25:d=0.5";
const HDR10_CLIP: &str = "format=yuv420p10le,setparams=color_primaries=bt2020:color_trc=smpte2084:colorspace=bt2020nc:range=tv";
const HDR10_X265: &str = "hdr10=1:repeat-headers=1:colorprim=bt2020:transfer=smpte2084:colormatrix=bt2020nc:master-display=G(13250,34500)B(7500,3000)R(34000,16000)WP(15635,16450)L(10000000,1):max-cll=1000,400:log-level=error";

/// The device routes the host proved, for the device it settled on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Routes {
    pub device: bool,
    pub download: bool,
}

impl Routes {
    pub const NONE: Self = Self {
        device: false,
        download: false,
    };
}

pub(super) fn validate(accel: HwAccel, filters: Filters) -> Routes {
    if !accel.is_device() {
        return Routes::NONE;
    }
    let Some(map) = ToneMap::resolve(ToneSource::Pq, filters) else {
        return Routes::NONE;
    };
    if accel == HwAccel::VideoToolbox {
        return Routes {
            device: false,
            download: true,
        };
    }
    let Some(clip) = Clip::make() else {
        return Routes::NONE;
    };
    std::thread::scope(|scope| {
        let device = scope.spawn(|| {
            accel.has_device_tonemap() && works(accel, map.routed(Route::Device), clip.path())
        });
        let download = works(accel, map.routed(Route::Download), clip.path());
        Routes {
            device: device.join().unwrap_or(false),
            download,
        }
    })
}

fn works(accel: HwAccel, map: ToneMap, clip: &Path) -> bool {
    let pipeline = Pipeline {
        accel,
        effort: Effort::Quality,
        tonemap: Some(map),
        frame: Some(CLIP_SIZE),
    };
    let mut cmd = Command::new("ffmpeg");
    input::without_proxies(&mut cmd);
    cmd.args(["-v", "error", "-nostdin"])
        .args(pipeline.input_args())
        .args(input::media(&MediaInput::Local(clip.to_path_buf())))
        .args(["-frames:v", "3"]);
    if let Some(filter) = pipeline.video_filter(Some(TARGET)) {
        cmd.args(["-vf", &filter]);
    }
    cmd.args(accel.encoder_args(Effort::Quality))
        .args(pipeline.output_tags())
        .args(["-f", "null", "-"]);
    crate::ffmpeg_run::run_capturing(cmd, ROUTE_TIMEOUT).is_ok()
}

pub(super) struct Clip(PathBuf);

impl Clip {
    pub(super) fn make() -> Option<Self> {
        if !contains(&ffmpeg_output(&["-hide_banner", "-encoders"]), b" libx265 ") {
            return None;
        }
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("kroma-tonemap-{}-{seq}.mkv", std::process::id()));
        let mut cmd = Command::new("ffmpeg");
        cmd.args(["-v", "error", "-nostdin", "-y"])
            .args(input::generated(CLIP_SOURCE))
            .args(["-vf", HDR10_CLIP, "-c:v", "libx265", "-preset", "ultrafast"])
            .args(["-x265-params", HDR10_X265])
            .arg(&path);
        let made = crate::ffmpeg_run::run_capturing(cmd, ROUTE_TIMEOUT).is_ok();
        let clip = Self(path);
        made.then_some(clip)
    }

    pub(super) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Clip {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
