//! Narrowing a requested [`StreamMode`] to what the client says it can take:
//! the codecs it declared it decodes, the bit depth behind one of those names,
//! and the frame size its decoder holds.

use kroma_module_wire::{HdrFormat, VideoStream};

use crate::stream_copy;

use super::mode::SURROUND_MIN_CHANNELS;
use super::tonemap::{cost, Route};
use super::{AudioMode, Rung, StreamMode, VideoMode};

const H264_10BIT: &str = "h26410bit";
const H264: &str = "h264";

impl StreamMode {
    /// The mode whose audio will actually reach the speakers, given the selected
    /// track's codec and the comma-separated set the client declared it can decode
    /// (`None` = declared nothing, and the request stands). Only an audio copy is
    /// ever overridden, and only the audio axis moves.
    pub fn for_client_audio(self, codec: Option<&str>, client_decodes: Option<&str>) -> Self {
        self.for_client_track(codec, None, client_decodes)
    }

    /// [`Self::for_client_audio`] that also knows how many channels the track
    /// has. A surround track the client cannot decode is re-encoded as 5.1
    /// E-AC-3 when the client declared `eac3`, as AC-3 when it declared only
    /// `ac3`, and as stereo AAC otherwise, so a television keeps its surround
    /// bed instead of receiving a stereo downmix.
    pub fn for_client_track(
        self,
        codec: Option<&str>,
        channels: Option<u32>,
        client_decodes: Option<&str>,
    ) -> Self {
        match (self.audio, client_decodes) {
            (AudioMode::Copy, Some(set)) if !client_can_play(codec, set) => Self {
                audio: transcode_target(channels, set),
                ..self
            },
            _ => self,
        }
    }

    /// The mode whose picture the client can show in the dynamic range it was
    /// encoded in, given the comma-separated HDR formats it declared (`hdr10`,
    /// `hdr10plus`, `hlg`, `dv`; `None` = declared nothing, and the request
    /// stands; empty = SDR only). A source it cannot show is re-encoded, but
    /// only where `can_tone_map` says the re-encode will come out in SDR: a
    /// re-encode that kept the HDR curve would cost a transcode and look no
    /// better than the copy. The re-encode is a 1080p one, because a tone-map is
    /// never run at a larger size. A Dolby Vision profile 5 copy is signalled as
    /// Dolby Vision only for a client that declared `dv`. Only a video copy is
    /// overridden.
    pub fn for_client_hdr(
        self,
        video: Option<&VideoStream>,
        client_shows: Option<&str>,
        can_tone_map: bool,
    ) -> Self {
        let (Some(video), Some(set)) = (video, client_shows) else {
            return self;
        };
        match self.video {
            VideoMode::Copy | VideoMode::CopyDolby
                if can_tone_map && !client_can_show(video, set) =>
            {
                Self {
                    video: VideoMode::H264At(Rung::P1080),
                    ..self
                }
            }
            VideoMode::Copy if is_profile_5(video) && declares(set, "dv") => Self {
                video: VideoMode::CopyDolby,
                ..self
            },
            _ => self,
        }
    }

    /// The mode whose re-encode is the size a tone-map is run at: a full-size
    /// re-encode of a picture that will be tone-mapped is a 1080p one.
    pub fn within_tone_map_box(self, tone_mappable: bool) -> Self {
        match self.video {
            VideoMode::H264 if tone_mappable => Self {
                video: VideoMode::H264At(Rung::P1080),
                ..self
            },
            _ => self,
        }
    }

    /// The mode a tone-map the host can only run on the CPU can keep up with: a
    /// 4K source on a few-core host drops to the smallest rung, which the client
    /// accepts as surely as any other. Only a 1080p re-encode is lowered.
    pub fn for_tone_map_cost(self, source: Option<(u32, u32)>, route: Option<Route>) -> Self {
        let encode_on_the_cpu = !super::hwaccel::detect().is_device();
        self.for_tone_map_cost_on(source, route, encode_on_the_cpu, cores())
    }

    fn for_tone_map_cost_on(
        self,
        source: Option<(u32, u32)>,
        route: Option<Route>,
        encode_on_the_cpu: bool,
        cores: usize,
    ) -> Self {
        let (VideoMode::H264At(Rung::P1080), Some(source), Some(route)) =
            (self.video, source, route)
        else {
            return self;
        };
        Self {
            video: VideoMode::H264At(cost::affordable(source, route, encode_on_the_cpu, cores)),
            ..self
        }
    }

    /// The mode whose audio can sit beside the picture it is muxed with. Some
    /// audio needs a muxer setting that would also announce a Dolby Vision
    /// stream the copy no longer is (see [`crate::stream_copy`]), so there the
    /// soundtrack is re-encoded as it would be for a client that could not
    /// decode it.
    pub fn for_copied_picture(
        self,
        video: Option<&VideoStream>,
        codec: Option<&str>,
        channels: Option<u32>,
        client_decodes: Option<&str>,
    ) -> Self {
        let dolby = self.video == VideoMode::CopyDolby;
        match (self.video.copies(), self.audio, codec) {
            (true, AudioMode::Copy, Some(codec))
                if !stream_copy::can_copy_audio_beside(video, dolby, codec) =>
            {
                Self {
                    audio: transcode_target(channels, client_decodes.unwrap_or_default()),
                    ..self
                }
            }
            _ => self,
        }
    }

    /// The mode whose picture will actually reach the screen, read the same way as
    /// [`Self::for_client_audio`] from the source video stream and the set the
    /// client declared. Two things play black on a copy: a codec the device has no
    /// decoder for, and H.264 above 8 bits, which the client declares as
    /// `h26410bit` beside `h264` because no codec name carries a depth. A depth
    /// nothing read is matched on the codec name alone. Only a video copy is
    /// overridden, and never speculatively - a client that declares nothing keeps
    /// its stream copy.
    pub fn for_client_video(
        self,
        codec: Option<&str>,
        bit_depth: Option<u32>,
        client_decodes: Option<&str>,
    ) -> Self {
        match (self.video, client_decodes) {
            (VideoMode::Copy | VideoMode::CopyDolby, Some(set))
                if !client_can_draw(codec, bit_depth, set) =>
            {
                Self {
                    video: VideoMode::H264,
                    ..self
                }
            }
            _ => self,
        }
    }

    /// The mode whose picture actually fits the decoder it is going to. A decoder
    /// has a size ceiling as surely as it has a codec list, and no player works
    /// around one: a frame over it either refuses to open or falls to a software
    /// decoder that cannot keep up. `source` is the file's own size and `max`
    /// what the client declared; either absent leaves the mode alone.
    ///
    /// Both axes count. Overrides a copy AND a full-size re-encode, because a
    /// re-encode at the source's size is just as unplayable as the source.
    pub fn for_client_frame(self, source: Option<(u32, u32)>, max: Option<(u32, u32)>) -> Self {
        let (Some(source), Some(max)) = (source, max) else {
            return self;
        };
        if source.0 <= max.0 && source.1 <= max.1 {
            return self;
        }
        Rung::under(max).map_or(self, |rung| Self {
            video: VideoMode::H264At(rung),
            ..self
        })
    }
}

fn transcode_target(channels: Option<u32>, client_set: &str) -> AudioMode {
    transcode_target_with(channels, client_set, crate::capabilities::audio_targets())
}

fn transcode_target_with(
    channels: Option<u32>,
    client_set: &str,
    encoders: crate::capabilities::AudioTargets,
) -> AudioMode {
    if !channels.is_some_and(|c| c >= SURROUND_MIN_CHANNELS) {
        AudioMode::Aac
    } else if encoders.eac3 && declares(client_set, "eac3") {
        AudioMode::Eac3
    } else if encoders.ac3 && declares(client_set, "ac3") {
        AudioMode::Ac3
    } else {
        AudioMode::Aac
    }
}

fn shown_by(video: &VideoStream) -> &'static [&'static str] {
    match video.hdr_format {
        None => &[],
        Some(HdrFormat::Hdr10) => &["hdr10"],
        Some(HdrFormat::Hdr10Plus) => &["hdr10plus", "hdr10"],
        Some(HdrFormat::Hlg) => &["hlg"],
        Some(HdrFormat::DolbyVision) => match (
            video.dolby_vision_compatibility,
            video.dolby_vision_profile,
        ) {
            (Some(2), _) => &[],
            (Some(0), _) | (None, Some(5)) => &["dv"],
            (Some(4), _) => &["dv", "hlg"],
            _ => &["dv", "hdr10"],
        },
    }
}

fn client_can_show(video: &VideoStream, client_set: &str) -> bool {
    let needs = shown_by(video);
    needs.is_empty() || needs.iter().any(|format| declares(client_set, format))
}

// An unprobed stream is assumed playable: with no codec there is nothing honest
// to override, and forcing a transcode would punish the common case.
fn client_can_play(codec: Option<&str>, client_set: &str) -> bool {
    let Some(codec) = codec else { return true };
    declares(client_set, codec)
}

fn client_can_draw(codec: Option<&str>, bit_depth: Option<u32>, client_set: &str) -> bool {
    let Some(codec) = codec else { return true };
    declares(client_set, declared_as(codec, bit_depth))
}

// HEVC is deliberately absent: its 10-bit profile has no token of its own, so a
// client that cannot draw one declares no `hevc` at all. Adding a token for it
// would hand a NEW client on an OLD server the black picture it declares its way
// out of today.
fn declared_as(codec: &str, bit_depth: Option<u32>) -> &str {
    if codec.eq_ignore_ascii_case(H264) && bit_depth.is_some_and(|depth| depth > 8) {
        H264_10BIT
    } else {
        codec
    }
}

fn is_profile_5(video: &VideoStream) -> bool {
    video.codec == "hevc" && video.dolby_vision_profile == Some(5)
}

fn cores() -> usize {
    std::thread::available_parallelism().map_or(4, std::num::NonZeroUsize::get)
}

pub(crate) fn declares(client_set: &str, token: &str) -> bool {
    client_set
        .split(',')
        .any(|declared| declared.trim().eq_ignore_ascii_case(token))
}

#[cfg(test)]
mod tests;
