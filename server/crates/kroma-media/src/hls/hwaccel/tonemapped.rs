//! The argument vectors of a tone-mapped re-encode, per device and [`Route`].
//!
//! Every route shrinks the picture to at most 1080p before anything maps it. A
//! device that decodes keeps decoding, and its own scaler does the shrinking, so
//! the CPU never sees a 4K frame it did not have to. Only the device vectors the
//! host proved at boot are ever used (see `tonemap::route`).

use super::super::tonemap::{Route, ToneMap};
use super::{owned, without_device_frames, HwAccel, Pipeline};

const TONE_BOX: (u32, u32) = (1920, 1080);
const DOWNLOAD: &str = "hwdownload,format=p010le";

enum Target {
    Exact(u32, u32),
    Box(u32, u32),
}

pub(super) fn within_box(size: Option<(u32, u32)>) -> (u32, u32) {
    size.map_or(TONE_BOX, |(w, h)| (w.min(TONE_BOX.0), h.min(TONE_BOX.1)))
}

fn fit_inside(source: (u32, u32), limit: (u32, u32)) -> Option<(u32, u32)> {
    if source.0 <= limit.0 && source.1 <= limit.1 {
        return None;
    }
    let scale =
        (f64::from(limit.0) / f64::from(source.0)).min(f64::from(limit.1) / f64::from(source.1));
    let even = |side: u32| ((f64::from(side) * scale) as u32 & !1).max(2);
    Some((even(source.0), even(source.1)))
}

impl Pipeline {
    pub(super) fn maps_on_the_cpu(self) -> bool {
        self.tonemap
            .is_some_and(|map| self.route(map) != Route::Device)
    }

    fn route(self, map: ToneMap) -> Route {
        if self.accel.is_device() && self.frame.is_some() {
            map.route()
        } else {
            Route::Cpu
        }
    }

    fn target(self, size: Option<(u32, u32)>) -> Option<Target> {
        let limit = within_box(size);
        match self.frame {
            Some(frame) => fit_inside(frame, limit).map(|(w, h)| Target::Exact(w, h)),
            None => Some(Target::Box(limit.0, limit.1)),
        }
    }

    pub(super) fn tonemapped_input(self, map: ToneMap) -> Vec<String> {
        match (self.route(map), self.accel) {
            (Route::Device, accel) => owned(accel.input_args(self.effort)),
            (Route::Download, HwAccel::Qsv | HwAccel::Vaapi) => {
                let mut args = owned(self.accel.input_args(self.effort));
                args.extend(self.accel.upload_device_args());
                args
            }
            (Route::Download, accel) => owned(accel.input_args(self.effort)),
            (Route::Cpu, HwAccel::Qsv | HwAccel::Vaapi) => {
                let mut args = owned(HwAccel::Software.input_args(self.effort));
                args.extend(self.accel.upload_device_args());
                args
            }
            (Route::Cpu, HwAccel::VideoToolbox | HwAccel::Nvenc) => {
                without_device_frames(self.accel.input_args(self.effort))
            }
            (Route::Cpu, HwAccel::Software) => owned(HwAccel::Software.input_args(self.effort)),
        }
    }

    pub(super) fn tonemapped_filter(self, map: ToneMap, size: Option<(u32, u32)>) -> String {
        let target = self.target(size);
        let mut steps: Vec<String> = Vec::new();
        match (self.route(map), self.accel) {
            (Route::Device, accel) => {
                if accel != HwAccel::Qsv {
                    steps.extend(device_scale(accel, &target));
                }
                steps.push(device_map(accel, &target));
            }
            (Route::Download, accel @ (HwAccel::Qsv | HwAccel::Vaapi | HwAccel::Nvenc)) => {
                steps.extend(device_scale(accel, &target));
                steps.push(DOWNLOAD.to_owned());
                steps.push(map.chain());
                steps.extend(accel.upload_filter().map(str::to_owned));
            }
            (_, accel) => {
                steps.extend(target.as_ref().map(|t| self.software_scale(t)));
                steps.push(map.chain());
                steps.extend(accel.upload_filter().map(str::to_owned));
            }
        }
        steps.join(",")
    }

    fn software_scale(self, target: &Target) -> String {
        let dims = match *target {
            Target::Exact(w, h) => format!("w={w}:h={h}"),
            Target::Box(w, h) => {
                format!("w={w}:h={h}:force_original_aspect_ratio=decrease:force_divisible_by=2")
            }
        };
        match self.effort.sws_flags() {
            Some(flags) => format!("scale={dims}:flags={flags}"),
            None => format!("scale={dims}"),
        }
    }
}

fn device_scale(accel: HwAccel, target: &Option<Target>) -> Option<String> {
    let Some(Target::Exact(w, h)) = target else {
        return None;
    };
    let filter = match accel {
        HwAccel::Vaapi => "scale_vaapi",
        HwAccel::Qsv => "scale_qsv",
        _ => "scale_cuda",
    };
    Some(format!("{filter}=w={w}:h={h}"))
}

fn device_map(accel: HwAccel, target: &Option<Target>) -> String {
    match accel {
        HwAccel::Qsv => {
            let size = match target {
                Some(Target::Exact(w, h)) => format!("w={w}:h={h}:"),
                _ => String::new(),
            };
            format!(
                "vpp_qsv={size}tonemap=1:format=nv12:out_color_matrix=bt709:\
                 out_color_primaries=bt709:out_color_transfer=bt709"
            )
        }
        HwAccel::Nvenc => {
            "tonemap_cuda=format=yuv420p:p=bt709:t=bt709:m=bt709:tonemap=bt2390".to_owned()
        }
        _ => "tonemap_vaapi=format=nv12:t=bt709:m=bt709:p=bt709".to_owned(),
    }
}

/// The scale and tone-map of a download, which runs on the CPU: the picture is
/// shrunk to at most 1080p first, exactly as a session's is.
pub(crate) fn tone_map_filter(map: ToneMap, frame: Option<(u32, u32)>) -> String {
    let map = map.on_the_cpu();
    Pipeline {
        accel: HwAccel::Software,
        effort: super::Effort::Quality,
        tonemap: Some(map),
        frame,
    }
    .tonemapped_filter(map, None)
}
