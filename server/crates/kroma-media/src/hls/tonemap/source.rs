//! The signal a source picture is encoded in.

use kroma_module_wire::{HdrFormat, VideoStream};

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
            Some(HdrFormat::DolbyVision) => {
                match (video.dolby_vision_compatibility, video.dolby_vision_profile) {
                    (Some(0), _) | (None, Some(5)) => Some(Self::DolbyVision),
                    (Some(2), _) => None,
                    (Some(4), _) => Some(Self::Hlg),
                    _ => by_transfer().or(Some(Self::Pq)),
                }
            }
            Some(HdrFormat::Hdr10 | HdrFormat::Hdr10Plus) => Some(Self::Pq),
            Some(HdrFormat::Hlg) => Some(Self::Hlg),
            None => by_transfer(),
        }
    }

    pub(super) const fn transfer(self) -> &'static str {
        match self {
            Self::Hlg => "arib-std-b67",
            Self::Pq | Self::DolbyVision => "smpte2084",
        }
    }
}
