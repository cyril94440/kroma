//! The muxer flags that make a stream copy into MP4 keep what makes it HDR,
//! Dolby Vision or Atmos, for the HLS program and the download alike.

use kroma_module_wire::VideoStream;

const HEVC: &str = "hevc";
const TRUEHD: &str = "truehd";

/// The output flags for a video stream copy, after `-c:v copy`.
///
/// HEVC is tagged `hvc1`, which Apple decoders require and `hev1` (what a copy
/// from MKV defaults to) is not. Dolby Vision profile 5 has no compatible base
/// layer: it takes `dvh1` only where `dolby` says the client declared a Dolby
/// Vision decoder, because every other decoder fails on `dvh1` where it plays
/// the stream as plain HEVC, in the wrong colours. Profile 8 keeps `hvc1` and
/// carries its `dvvC` box, so a DV decoder engages and every other player falls
/// back on the HDR10 or HLG base. A single track profile 7 is not valid in MP4:
/// its enhancement layer and RPU are stripped and it is signalled as the HDR10
/// its base layer is.
pub fn video_args(video: Option<&VideoStream>, dolby: bool) -> Vec<&'static str> {
    let Some(video) = video.filter(|v| v.codec == HEVC) else {
        return Vec::new();
    };
    match video.dolby_vision_profile {
        Some(5) if dolby => vec!["-tag:v", "dvh1"],
        Some(7) => vec![
            "-tag:v",
            "hvc1",
            "-bsf:v",
            "filter_units=remove_types=62|63",
        ],
        _ => vec!["-tag:v", "hvc1"],
    }
}

/// The `-strict` level the copy needs, if any. ffmpeg writes the Dolby Vision
/// configuration box only at `unofficial` and muxes TrueHD into MP4 only at
/// `-2` (experimental), which includes it, so the lower of the two wins.
pub fn strictness(
    video: Option<&VideoStream>,
    dolby: bool,
    audio_codec: Option<&str>,
) -> Option<&'static str> {
    if audio_codec == Some(TRUEHD) {
        return Some("-2");
    }
    let carries_dolby_box = video.is_some_and(|v| {
        v.codec == HEVC
            && match v.dolby_vision_profile {
                Some(5) => dolby,
                Some(8) => true,
                _ => false,
            }
    });
    carries_dolby_box.then_some("unofficial")
}

/// Whether `audio_codec` can be copied beside this video copy. TrueHD needs
/// `-strict -2`, which also writes the Dolby Vision box from the stream's own
/// configuration record, and ffmpeg 6.1 has no way to drop that record: a
/// profile 7 whose layers were just stripped, or a profile 5 not signalled as
/// Dolby, would be announced as a Dolby Vision stream it no longer is.
pub fn can_copy_audio_beside(
    video: Option<&VideoStream>,
    dolby: bool,
    audio_codec: &str,
) -> bool {
    let announces_what_it_dropped = video.is_some_and(|v| {
        v.codec == HEVC
            && match v.dolby_vision_profile {
                Some(5) => !dolby,
                Some(7) => true,
                _ => false,
            }
    });
    !(audio_codec == TRUEHD && announces_what_it_dropped)
}

#[cfg(test)]
mod tests;
