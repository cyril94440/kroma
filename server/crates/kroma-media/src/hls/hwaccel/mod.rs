//! What this host can hand a picture to.
//!
//! A re-encode is the one thing in the delivery path that costs real CPU, so it
//! is the one thing worth putting on fixed-function silicon: an Intel iGPU, an
//! Apple VideoToolbox block or an NVIDIA encoder turns a 4K→1080p downscale from
//! "one stream saturates the box" into single-digit percent. Probed once per
//! process, because the answer cannot change while it runs.
//!
//! [`probe`] is where the answer comes from, and why it is worth reporting: a
//! device that is present, listed and unusable looks exactly like no device at
//! all from the outside, and the operator is the only one who can fix it.

mod probe;
mod tonemapped;

use std::sync::OnceLock;

use super::software::Effort;
use super::tonemap::{ToneMap, SDR_TAGS};

const TONEMAP_SLOTS: usize = 2;

pub use probe::{detect, detected, prime, Detection};

/// What one re-encode runs on: the device that shrinks the picture and, where
/// that device is the CPU, how hard it is allowed to work per frame. The two are
/// one decision - the tier only means anything on the software path - so they
/// are chosen and carried together. `tonemap` is set when the source is HDR and
/// this host can map it, and carries the [`Route`] it takes; `frame` is the size
/// of the source, which a device scaler needs spelled out.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Pipeline {
    pub accel: HwAccel,
    pub effort: Effort,
    pub tonemap: Option<ToneMap>,
    pub frame: Option<(u32, u32)>,
}

impl Pipeline {
    /// The device this host has, and the tier this job can afford on it, from the
    /// picture going in and the box it is fitted into. A source nobody measured
    /// cannot be scaled on a device, so its tone-map stays on the CPU.
    pub fn choose(
        source: Option<(u32, u32)>,
        target: Option<(u32, u32)>,
        tonemap: Option<ToneMap>,
    ) -> Self {
        let tonemap = tonemap.map(|map| if source.is_some() { map } else { map.on_the_cpu() });
        let target = tonemap.map_or(target, |_| Some(tonemapped::within_box(target)));
        Self {
            accel: detect(),
            effort: Effort::choose(source, target, tonemap.is_some()),
            tonemap,
            frame: source,
        }
    }

    /// Args that must precede `-i`.
    pub fn input_args(self) -> Vec<String> {
        match self.tonemap {
            None => owned(self.accel.input_args(self.effort)),
            Some(map) => self.tonemapped_input(map),
        }
    }

    /// The `-vf` for this job: the scale and depth change the device needs, or
    /// the scale and tone-map of a picture that is never larger than 1080p.
    pub fn video_filter(self, size: Option<(u32, u32)>) -> Option<String> {
        match self.tonemap {
            None => self.accel.video_filter(size, self.effort),
            Some(map) => Some(self.tonemapped_filter(map, size)),
        }
    }

    /// Tags a tone-mapped picture as the SDR it now is.
    pub fn output_tags(self) -> &'static [&'static str] {
        if self.tonemap.is_some() {
            &SDR_TAGS
        } else {
            &[]
        }
    }

    /// Whether the frames are rebuilt by the CPU, which is the case where the box
    /// has to be taken away from background work: a software encode, or a
    /// tone-map that any part of runs on the CPU.
    pub fn on_the_cpu(self) -> bool {
        !self.accel.is_device() || self.maps_on_the_cpu()
    }

    /// How many of the host's re-encode slots this session takes. A tone-map on
    /// the CPU is a float pipeline over every pixel and fills a small host alone.
    pub fn slots(self) -> usize {
        if self.maps_on_the_cpu() {
            TONEMAP_SLOTS
        } else {
            1
        }
    }
}

/// The pipeline the picture takes when it has to be re-encoded. `Software` is
/// the floor, not a failure: it is what a host with no usable device gets.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HwAccel {
    VideoToolbox,
    Qsv,
    Vaapi,
    Nvenc,
    Software,
}

/// The Linux render node VAAPI and QSV open when nothing says otherwise. A host
/// with several (an iGPU beside a card) names the one it means with
/// `KROMA_RENDER_NODE`, because that choice belongs to an operator, not a probe.
const DEFAULT_RENDER_NODE: &str = "/dev/dri/renderD128";

pub(super) fn render_node() -> &'static str {
    static NODE: OnceLock<&str> = OnceLock::new();
    NODE.get_or_init(|| match std::env::var("KROMA_RENDER_NODE") {
        Ok(v) if !v.trim().is_empty() => Box::leak(v.trim().to_owned().into_boxed_str()),
        _ => DEFAULT_RENDER_NODE,
    })
}

fn owned(args: Vec<&'static str>) -> Vec<String> {
    args.into_iter().map(str::to_owned).collect()
}

fn without_device_frames(args: Vec<&'static str>) -> Vec<String> {
    let mut kept = Vec::new();
    let mut rest = args.into_iter();
    while let Some(arg) = rest.next() {
        if arg == "-hwaccel_output_format" {
            rest.next();
        } else {
            kept.push(arg.to_owned());
        }
    }
    kept
}

impl HwAccel {
    /// How it reads in a log line or an admin field.
    pub const fn label(self) -> &'static str {
        match self {
            Self::VideoToolbox => "videotoolbox",
            Self::Qsv => "qsv",
            Self::Vaapi => "vaapi",
            Self::Nvenc => "nvenc",
            Self::Software => "software",
        }
    }

    /// Whether the picture is shrunk by fixed-function silicon rather than by
    /// the CPU, which is what decides whether the effort tier matters at all.
    pub const fn is_device(self) -> bool {
        !matches!(self, Self::Software)
    }

    /// Whether a filter of this device's own could map HDR to SDR, to be tried at
    /// boot: nothing about a driver says it will work until it has.
    pub const fn has_device_tonemap(self) -> bool {
        matches!(self, Self::Qsv | Self::Vaapi | Self::Nvenc)
    }

    /// Args that must precede `-i`: they choose the decoder and say which memory
    /// the decoded frames land in. `effort` only reaches the software path, whose
    /// decoder is the CPU.
    pub fn input_args(self, effort: Effort) -> Vec<&'static str> {
        match self {
            Self::VideoToolbox => vec!["-hwaccel", "videotoolbox"],
            Self::Qsv => vec!["-hwaccel", "qsv", "-hwaccel_output_format", "qsv"],
            Self::Vaapi => vec![
                "-hwaccel",
                "vaapi",
                "-hwaccel_device",
                render_node(),
                "-hwaccel_output_format",
                "vaapi",
            ],
            Self::Nvenc => vec!["-hwaccel", "cuda", "-hwaccel_output_format", "cuda"],
            Self::Software => effort.decoder_args().to_vec(),
        }
    }

    /// Global args that declare the device a filter graph uploads system frames
    /// to, empty where the encoder takes system frames itself.
    pub fn upload_device_args(self) -> Vec<String> {
        let node = render_node();
        match self {
            Self::Vaapi => vec!["-vaapi_device".to_owned(), node.to_owned()],
            Self::Qsv => [
                "-init_hw_device",
                &format!("vaapi=va:{node}"),
                "-init_hw_device",
                "qsv=hw@va",
                "-filter_hw_device",
                "hw",
            ]
            .map(str::to_owned)
            .to_vec(),
            Self::VideoToolbox | Self::Nvenc | Self::Software => Vec::new(),
        }
    }

    /// The filter that moves system-memory frames into the memory this encoder
    /// reads, or None where it takes them as they are.
    pub const fn upload_filter(self) -> Option<&'static str> {
        match self {
            Self::Qsv | Self::Vaapi => Some("format=nv12,hwupload"),
            Self::VideoToolbox | Self::Nvenc | Self::Software => None,
        }
    }

    /// The `-vf` this device needs to reach the encoder, or None where the
    /// picture already arrives in a shape it takes.
    ///
    /// Given a box, the picture is fitted inside it without changing shape:
    /// `decrease` is what makes it a box rather than a size, so a scope frame
    /// comes out width-limited and short and a 16:9 one height-limited, and
    /// `force_divisible_by=2` because H.264 has no odd dimensions.
    ///
    /// With or without one, a device pipeline must still be told the format to
    /// hand over: the decoder keeps a 10-bit source in 10-bit surfaces and none
    /// of these encoders takes 10-bit H.264, so the filter is where the depth is
    /// dropped. The software and VideoToolbox paths carry frames in system
    /// memory, where the encoder's own `-pix_fmt` does it instead.
    ///
    /// Each device scales in the memory it decoded into, which is why this is
    /// five filters and not one with a flag.
    pub fn video_filter(self, size: Option<(u32, u32)>, effort: Effort) -> Option<String> {
        let fit = size.map(|(w, h)| {
            format!("w={w}:h={h}:force_original_aspect_ratio=decrease:force_divisible_by=2")
        });
        let device = |name: &str, format: &str| {
            Some(match &fit {
                Some(fit) => format!("{name}={fit}:format={format}"),
                None => format!("{name}=format={format}"),
            })
        };
        match self {
            // Frames leave VideoToolbox for the scaler, so this one runs on the
            // CPU: it is the only part of the pipeline that does.
            Self::VideoToolbox | Self::Software => {
                fit.as_ref().map(|fit| match effort.sws_flags() {
                    Some(flags) => format!("scale={fit}:flags={flags}"),
                    None => format!("scale={fit}"),
                })
            }
            Self::Qsv => device("scale_qsv", "nv12"),
            Self::Vaapi => device("scale_vaapi", "nv12"),
            Self::Nvenc => device("scale_cuda", "yuv420p"),
        }
    }

    /// The encoder and its rate control. 8-bit H.264 whatever the device: the
    /// re-encode exists for players the source defeats, so it must land on the
    /// one profile every target reads.
    pub fn encoder_args(self, effort: Effort) -> Vec<&'static str> {
        match self {
            // VideoToolbox has no CRF: quality is a bitrate, and `-realtime`
            // keeps the encoder from buffering a whole GOP ahead of the player.
            Self::VideoToolbox => vec![
                "-c:v",
                "h264_videotoolbox",
                "-realtime",
                "1",
                "-b:v",
                "8000k",
                "-pix_fmt",
                "yuv420p",
            ],
            Self::Qsv => vec![
                "-c:v",
                "h264_qsv",
                "-global_quality",
                "23",
                "-look_ahead",
                "0",
            ],
            Self::Vaapi => vec!["-c:v", "h264_vaapi", "-qp", "23"],
            Self::Nvenc => vec![
                "-c:v",
                "h264_nvenc",
                "-preset",
                "p4",
                "-rc",
                "vbr",
                "-cq",
                "23",
            ],
            Self::Software => {
                let mut args = vec!["-c:v", "libx264"];
                args.extend_from_slice(effort.x264_args());
                args.extend_from_slice(&["-pix_fmt", "yuv420p"]);
                args
            }
        }
    }
}

#[cfg(test)]
mod tests;
