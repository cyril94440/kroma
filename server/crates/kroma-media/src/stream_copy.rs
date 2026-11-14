//! The muxer flags that make a stream copy into MP4 keep what makes it HDR,
//! Dolby Vision or Atmos, for the HLS program and the download alike.

use kroma_module_wire::VideoStream;

const HEVC: &str = "hevc";

/// The output flags for a video stream copy, after `-c:v copy`.
///
/// HEVC is tagged `hvc1`, which Apple decoders require and `hev1` (what a copy
/// from MKV defaults to) is not. Dolby Vision profile 5 has no compatible base
/// layer and takes `dvh1`, so a DV decoder engages and nothing else pretends to
/// read it; profile 8 keeps `hvc1` and carries its `dvvC` box, so a DV decoder
/// engages and every other player falls back on the HDR10 or HLG base. A single
/// track profile 7 is not valid in MP4: its enhancement layer and RPU are
/// stripped and it is signalled as the HDR10 its base layer is.
pub fn video_args(video: Option<&VideoStream>) -> Vec<&'static str> {
    let Some(video) = video.filter(|v| v.codec == HEVC) else {
        return Vec::new();
    };
    match video.dolby_vision_profile {
        Some(5) => vec!["-tag:v", "dvh1"],
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
pub fn strictness(video: Option<&VideoStream>, audio_codec: Option<&str>) -> Option<&'static str> {
    if audio_codec == Some("truehd") {
        return Some("-2");
    }
    let carries_dolby_box = video.is_some_and(|v| {
        v.codec == HEVC && matches!(v.dolby_vision_profile, Some(5 | 8))
    });
    carries_dolby_box.then_some("unofficial")
}

#[cfg(test)]
mod tests;
