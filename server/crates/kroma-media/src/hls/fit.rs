//! Narrowing a requested [`StreamMode`] to what the client says it can take:
//! the codecs it declared it decodes, the bit depth behind one of those names,
//! and the frame size its decoder holds.

use kroma_module_wire::{HdrFormat, VideoStream};

use super::mode::SURROUND_MIN_CHANNELS;
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
    /// better than the copy. Only a video copy is overridden.
    pub fn for_client_hdr(
        self,
        video: Option<&VideoStream>,
        client_shows: Option<&str>,
        can_tone_map: bool,
    ) -> Self {
        match (self.video, video, client_shows) {
            (VideoMode::Copy, Some(video), Some(set))
                if can_tone_map && !client_can_show(video, set) =>
            {
                Self {
                    video: VideoMode::H264,
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
            (VideoMode::Copy, Some(set)) if !client_can_draw(codec, bit_depth, set) => Self {
                video: VideoMode::H264,
                ..self
            },
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
    if !channels.is_some_and(|c| c >= SURROUND_MIN_CHANNELS) {
        AudioMode::Aac
    } else if declares(client_set, "eac3") {
        AudioMode::Eac3
    } else if declares(client_set, "ac3") {
        AudioMode::Ac3
    } else {
        AudioMode::Aac
    }
}

// The formats any one of which lets a client show the picture, as the base
// layer of a Dolby Vision stream falls back on the format its compatibility id
// names. Empty where the picture needs no HDR at all.
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

fn declares(client_set: &str, token: &str) -> bool {
    client_set
        .split(',')
        .any(|declared| declared.trim().eq_ignore_ascii_case(token))
}

#[cfg(test)]
mod tests;
