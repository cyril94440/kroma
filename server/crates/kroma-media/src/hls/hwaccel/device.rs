//! What each device runs between the decoder and the encoder, and the encoder
//! itself.

use super::super::software::Effort;
use super::HwAccel;

impl HwAccel {
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
