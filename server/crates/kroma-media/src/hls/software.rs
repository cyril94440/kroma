//! What a host with no encoder device can afford to spend per frame.
//!
//! On fixed-function silicon a downscale costs the same whatever it is set to,
//! and the question never arises. libx264 is the case where the box either stays
//! ahead of the player or does not, and the cost is set by the pixels crossing
//! the pipeline rather than by the title: a 4K source decoded down to 1080p is
//! four times the decode of a 1080p one and lands on the same encoder.

/// How much the software pipeline may spend per frame.
///
/// `Quality` is what a box with headroom uses. `Realtime` buys throughput with a
/// softer downscale and a slightly larger stream, which is the right trade on the
/// path it lives on: a re-encode only exists for a device that cannot play the
/// source at all, so a picture that arrives is worth more than a picture that is
/// 10% better and late.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Effort {
    Quality,
    Realtime,
}

// Frame rate assumed of a source nobody probed. 30 rather than 24: reading the
// load high costs a slightly softer picture, reading it low costs playback.
const ASSUMED_FPS: f64 = 30.0;
// Megapixels a second one logical core is trusted with before the tier drops.
// Set low on purpose: reading a box as weaker than it is costs a softer picture
// on a fallback path, reading it as stronger than it is costs playback, and the
// box this has to hold for is a four-thread 1.5 GHz Celeron.
const CORE_BUDGET: f64 = 15.0;
// What a tone-mapped megapixel costs beyond the plain one, in plain megapixels:
// the float chain measured 1.3 times a libx264 veryfast encode of the same frame.
const TONEMAP_EXTRA_MPIX: f64 = 1.3;

impl Effort {
    /// The tier for one transcode, from the picture going in, the box it is
    /// fitted into (`None` = the source's own size) and this host's core count.
    /// `KROMA_ENCODE_EFFORT=quality|realtime` pins it, which is how an operator
    /// rules the tier out of a problem.
    ///
    /// A source nobody probed reads as `Realtime`: it may well be the 4K the
    /// re-encode exists for, and guessing low here is the guess that stalls.
    pub fn choose(
        source: Option<(u32, u32)>,
        target: Option<(u32, u32)>,
        tonemapped: bool,
    ) -> Self {
        match std::env::var("KROMA_ENCODE_EFFORT").ok().as_deref() {
            Some("quality") => return Self::Quality,
            Some("realtime") => return Self::Realtime,
            _ => {}
        }
        let Some(source) = source else {
            return Self::Realtime;
        };
        Self::for_load(load(source, target.unwrap_or(source), cores(), tonemapped))
    }

    /// The decoder flags this tier asks for, which precede `-i`.
    ///
    /// `noref` is the conservative rung of `-skip_loop_filter`: deblocking is
    /// dropped only on frames nothing else is predicted from, so the artefact it
    /// leaves cannot propagate, and a 2:1 downscale washes out most of what the
    /// filter would have smoothed. It is the only lever the decode side has, and
    /// on a 4K source the decode is the half that does not fit.
    pub const fn decoder_args(self) -> &'static [&'static str] {
        match self {
            Self::Realtime => &["-skip_loop_filter", "noref"],
            Self::Quality => &[],
        }
    }

    /// The swscale kernel for the downscale, or None to leave ffmpeg its default
    /// (bicubic). Only the paths that scale in system memory read this.
    pub const fn sws_flags(self) -> Option<&'static str> {
        match self {
            Self::Realtime => Some("fast_bilinear"),
            Self::Quality => None,
        }
    }

    /// libx264's rate control and speed.
    ///
    /// `ultrafast` is not one of the options on purpose: it turns CABAC off, and
    /// the stream it makes at a fixed CRF is large enough that the muxer, the disk
    /// and the LAN give back much of what the cheaper analysis won - on a path
    /// that exists because the client was already the constrained end.
    pub const fn x264_args(self) -> &'static [&'static str] {
        match self {
            Self::Quality => &["-preset", "veryfast", "-crf", "21"],
            Self::Realtime => &["-preset", "superfast", "-crf", "23"],
        }
    }

    /// How it reads in a log line.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Quality => "quality",
            Self::Realtime => "realtime",
        }
    }

    const fn for_load(load: f64) -> Self {
        if load <= CORE_BUDGET {
            Self::Quality
        } else {
            Self::Realtime
        }
    }
}

fn load(source: (u32, u32), target: (u32, u32), cores: usize, tonemapped: bool) -> f64 {
    let mpix = |(w, h): (u32, u32)| f64::from(w) * f64::from(h) / 1_000_000.0;
    let mapped = if tonemapped { 1.0 + TONEMAP_EXTRA_MPIX } else { 1.0 };
    (mpix(source) + mpix(target) * mapped) * ASSUMED_FPS / cores.max(1) as f64
}

fn cores() -> usize {
    std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(4)
}

#[cfg(test)]
mod tests {
    use super::*;

    const UHD: (u32, u32) = (3840, 2160);
    const HD: (u32, u32) = (1920, 1080);

    #[test]
    fn a_four_core_nas_downscaling_4k_cannot_afford_the_quality_tier() {
        // A DS918+: four threads of 1.5 GHz Celeron against 10 megapixels a frame.
        assert_eq!(Effort::for_load(load(UHD, HD, 4, false)), Effort::Realtime);
    }

    #[test]
    fn a_box_with_headroom_keeps_the_quality_tier() {
        assert_eq!(Effort::for_load(load(HD, HD, 12, false)), Effort::Quality);
    }

    #[test]
    fn the_same_job_gets_cheaper_as_cores_are_added() {
        assert!(load(UHD, HD, 16, false) < load(UHD, HD, 4, false));
    }

    #[test]
    fn an_unprobed_source_reads_as_the_4k_the_re_encode_exists_for() {
        assert_eq!(Effort::choose(None, Some(HD), false), Effort::Realtime);
    }

    #[test]
    fn a_downscale_costs_more_than_the_same_picture_left_alone() {
        // The source is decoded either way, so the target is the only term that moves.
        assert!(load(UHD, HD, 4, false) < load(UHD, UHD, 4, false));
    }

    #[test]
    fn a_tone_mapped_picture_asks_more_of_the_box_than_the_same_one_left_alone() {
        assert!(load(HD, HD, 12, true) > load(HD, HD, 12, false));
        assert_eq!(Effort::for_load(load(HD, HD, 12, false)), Effort::Quality);
        assert_eq!(Effort::for_load(load(HD, HD, 12, true)), Effort::Realtime);
    }

    #[test]
    fn the_realtime_tier_is_cheaper_at_every_stage() {
        assert!(!Effort::Realtime.decoder_args().is_empty());
        assert!(Effort::Quality.decoder_args().is_empty());
        assert_eq!(Effort::Realtime.sws_flags(), Some("fast_bilinear"));
        assert_eq!(Effort::Quality.sws_flags(), None);
        assert!(Effort::Realtime.x264_args().contains(&"superfast"));
        assert!(Effort::Quality.x264_args().contains(&"veryfast"));
    }

    #[test]
    fn no_tier_reaches_for_ultrafast() {
        for effort in [Effort::Quality, Effort::Realtime] {
            assert!(
                !effort.x264_args().contains(&"ultrafast"),
                "{}",
                effort.label()
            );
        }
    }
}
