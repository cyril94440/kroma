//! How a program is produced on each axis, and the `{mode}` URL path segment
//! that spells it. The token is also part of the session key, so every distinct
//! value is another ffmpeg and another copy of the segments on disk.

/// The boxes a downscale fits the picture inside. A BOX rather than a height,
/// because a decoder's ceiling is two numbers and scope content hits the width
/// one first: 3840x1604 is under a 1920-tall limit and still four times too wide.
/// A closed set, because the box is part of the session key and every distinct
/// value is another ffmpeg and another copy of the segments on disk.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rung {
    P1080,
    P720,
}

impl Rung {
    /// Width, height. The picture keeps its aspect and fits INSIDE this, so a
    /// scope frame comes out wider and shorter than the box.
    pub const fn box_size(self) -> (u32, u32) {
        match self {
            Self::P1080 => (1920, 1080),
            Self::P720 => (1280, 720),
        }
    }

    /// The largest rung that fits inside a decoder capped at `max`, or None when
    /// even the smallest does not (a device no downscale reaches).
    pub(super) fn under(max: (u32, u32)) -> Option<Self> {
        [Self::P1080, Self::P720].into_iter().find(|r| {
            let (w, h) = r.box_size();
            w <= max.0 && h <= max.1
        })
    }

    const fn token(self) -> &'static str {
        match self {
            Self::P1080 => "h264-1080-",
            Self::P720 => "h264-720-",
        }
    }

    // The token without the trailing separator the audio axis is joined by.
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::P1080 => "h264-1080",
            Self::P720 => "h264-720",
        }
    }
}

/// One program's video treatment: stream-copy, a transcode to the 8-bit H.264
/// every target decodes, or that transcode scaled down to a rung the client's
/// decoder accepts. A picture is orders of magnitude dearer to re-encode than a
/// soundtrack, so [`StreamMode::for_client_video`] is the only thing that ever
/// reaches past `Copy`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VideoMode {
    Copy,
    H264,
    H264At(Rung),
}

impl VideoMode {
    /// The box the picture is fitted inside, or None where it keeps its own size.
    pub const fn box_size(self) -> Option<(u32, u32)> {
        match self {
            Self::H264At(rung) => Some(rung.box_size()),
            Self::Copy | Self::H264 => None,
        }
    }

    const fn token(self) -> &'static str {
        match self {
            Self::Copy => "",
            Self::H264 => H264_PREFIX,
            Self::H264At(rung) => rung.token(),
        }
    }
}

/// One program's audio treatment: stream-copy, plain stereo-AAC transcode, or an
/// AAC transcode with a loudness filter (night-mode volume leveling for clients
/// with no local audio DSP, e.g. Tizen AVPlay). A filter always transcodes - a
/// stream copy cannot be filtered - so the filtered variants subsume `Aac`.
///
/// `Eac3` and `Ac3` keep the 5.1 bed of a surround source a client cannot decode
/// as it is: 640 kbps E-AC-3 or AC-3, for a client that declared the codec. A
/// source with fewer than five channels is still encoded as stereo AAC, so the
/// mode is safe to ask for on any track. The loudness variants stay stereo AAC:
/// their compressor is tuned for a downmix, and they exist for clients that
/// cannot run one themselves.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AudioMode {
    Copy,
    Aac,
    AacStandard,
    AacNight,
    Eac3,
    Ac3,
}

/// The fewest channels a source has for its surround bed to be worth keeping.
pub const SURROUND_MIN_CHANNELS: u32 = 5;

impl AudioMode {
    const fn token(self) -> &'static str {
        match self {
            Self::Copy => "copy",
            Self::Aac => "aac",
            Self::AacStandard => "aac-standard",
            Self::AacNight => "aac-night",
            Self::Eac3 => "eac3",
            Self::Ac3 => "ac3",
        }
    }

    pub(super) const fn label(self) -> &'static str {
        self.token()
    }

    pub(super) fn transcode(self) -> bool {
        !matches!(self, Self::Copy)
    }

    // Tuned to match the client Web Audio compressor (packages/ui
    // `audio-filter.ts`) so every engine sounds the same.
    pub(super) fn filter_chain(self) -> Option<&'static str> {
        match self {
            Self::Copy | Self::Aac | Self::Eac3 | Self::Ac3 => None,
            Self::AacStandard => {
                Some("acompressor=threshold=0.063:ratio=4:attack=10:release=250:knee=6:makeup=1.4")
            }
            Self::AacNight => {
                Some("acompressor=threshold=0.04:ratio=8:attack=4:release=250:knee=5,volume=0.9")
            }
        }
    }
}

/// How one program is produced, on both axes. They are independent: a device can
/// decode the picture and not the soundtrack, or the reverse.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StreamMode {
    pub video: VideoMode,
    pub audio: AudioMode,
}

// The video axis is spelled as a prefix on the audio token, so every URL minted
// before the axis existed still parses - and still means a video stream copy.
const H264_PREFIX: &str = "h264-";

impl StreamMode {
    pub const fn new(video: VideoMode, audio: AudioMode) -> Self {
        StreamMode { video, audio }
    }

    /// Parse the `{mode}` URL path segment (also the token used in session keys).
    pub fn parse(s: &str) -> Option<Self> {
        // Longest prefix first: `h264-1080-` also starts with `h264-`.
        let (video, rest) = if let Some(rest) = s.strip_prefix(Rung::P1080.token()) {
            (VideoMode::H264At(Rung::P1080), rest)
        } else if let Some(rest) = s.strip_prefix(Rung::P720.token()) {
            (VideoMode::H264At(Rung::P720), rest)
        } else if let Some(rest) = s.strip_prefix(H264_PREFIX) {
            (VideoMode::H264, rest)
        } else {
            (VideoMode::Copy, s)
        };
        let audio = match rest {
            "copy" => AudioMode::Copy,
            "aac" => AudioMode::Aac,
            "aac-standard" => AudioMode::AacStandard,
            "aac-night" => AudioMode::AacNight,
            "eac3" => AudioMode::Eac3,
            "ac3" => AudioMode::Ac3,
            _ => return None,
        };
        Some(Self::new(video, audio))
    }

    // Inverse of `parse`; emitted by the client URL builder in `packages/client media.ts`.
    pub fn token(self) -> String {
        format!("{}{}", self.video.token(), self.audio.token())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIDEO_MODES: [VideoMode; 4] = [
        VideoMode::Copy,
        VideoMode::H264,
        VideoMode::H264At(Rung::P1080),
        VideoMode::H264At(Rung::P720),
    ];
    const AUDIO_MODES: [AudioMode; 6] = [
        AudioMode::Copy,
        AudioMode::Aac,
        AudioMode::AacStandard,
        AudioMode::AacNight,
        AudioMode::Eac3,
        AudioMode::Ac3,
    ];

    #[test]
    fn mode_tokens_round_trip() {
        for video in VIDEO_MODES {
            for audio in AUDIO_MODES {
                let mode = StreamMode::new(video, audio);
                assert_eq!(StreamMode::parse(&mode.token()), Some(mode));
            }
        }
        assert_eq!(StreamMode::parse("bogus"), None);
        assert_eq!(StreamMode::parse("h264-"), None);
        assert_eq!(StreamMode::parse("h264-bogus"), None);
    }

    #[test]
    fn a_token_with_no_video_prefix_still_means_a_video_copy() {
        for (token, audio) in [
            ("copy", AudioMode::Copy),
            ("aac", AudioMode::Aac),
            ("aac-standard", AudioMode::AacStandard),
            ("aac-night", AudioMode::AacNight),
            ("eac3", AudioMode::Eac3),
            ("ac3", AudioMode::Ac3),
        ] {
            assert_eq!(
                StreamMode::parse(token),
                Some(StreamMode::new(VideoMode::Copy, audio))
            );
        }
    }

    #[test]
    fn the_video_axis_is_a_prefix_on_the_audio_token() {
        assert_eq!(
            StreamMode::new(VideoMode::H264, AudioMode::Copy).token(),
            "h264-copy"
        );
        assert_eq!(
            StreamMode::new(VideoMode::H264, AudioMode::AacNight).token(),
            "h264-aac-night"
        );
    }

    #[test]
    fn the_surround_targets_are_named_after_their_codec_and_carry_the_video_prefix() {
        assert_eq!(
            StreamMode::new(VideoMode::Copy, AudioMode::Eac3).token(),
            "eac3"
        );
        assert_eq!(
            StreamMode::new(VideoMode::H264At(Rung::P1080), AudioMode::Ac3).token(),
            "h264-1080-ac3"
        );
        assert_eq!(
            StreamMode::parse("h264-720-eac3"),
            Some(StreamMode::new(VideoMode::H264At(Rung::P720), AudioMode::Eac3))
        );
    }

    #[test]
    fn the_surround_targets_transcode_without_a_loudness_filter() {
        for mode in [AudioMode::Eac3, AudioMode::Ac3] {
            assert!(mode.transcode());
            assert!(mode.filter_chain().is_none());
        }
    }

    #[test]
    fn filtered_modes_transcode_with_a_chain() {
        assert!(!AudioMode::Copy.transcode());
        assert!(AudioMode::Aac.transcode());
        assert!(AudioMode::Aac.filter_chain().is_none());
        assert!(AudioMode::AacStandard.transcode());
        assert!(AudioMode::AacStandard
            .filter_chain()
            .unwrap()
            .contains("ratio=4"));
        assert!(AudioMode::AacNight
            .filter_chain()
            .unwrap()
            .contains("ratio=8"));
    }
}
