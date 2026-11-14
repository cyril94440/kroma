//! Map ffprobe's output onto our best-effort [`ProbeResult`]: stream selection,
//! codec normalization, bit-depth and HDR-variant heuristics.

use kroma_module_wire::{
    AudioStream, ColorInfo, HdrFormat, SpatialAudio, SubtitleTrack, VideoStream,
};

use super::ffprobe_output::{FfChapter, FfStream, FfprobeOutput};
use super::{Chapter, ProbeResult};

pub(super) fn build_result(raw: FfprobeOutput) -> ProbeResult {
    // Container order: the audio-relative index is exactly ffmpeg's `0:a:<n>`.
    let audio_tracks: Vec<AudioStream> = raw
        .streams
        .iter()
        .filter(|&s| s.codec_type.as_deref() == Some("audio"))
        .enumerate()
        .map(|(i, s)| build_audio(s, i as u32))
        .collect();
    let audio = audio_tracks.first().cloned();

    ProbeResult {
        duration_ms: raw.format.as_ref().and_then(|fmt| {
            fmt.duration
                .as_deref()
                .and_then(|d| d.parse::<f64>().ok())
                .map(|secs| (secs * 1000.0) as u64)
        }),
        video: video_stream(&raw).map(build_video),
        audio,
        audio_tracks,
        subtitles: raw
            .streams
            .iter()
            .filter(|&s| s.codec_type.as_deref() == Some("subtitle"))
            .map(|s| SubtitleTrack {
                language: s.language(),
                codec: normalize_codec(s.codec_name.as_deref()),
                title: s.title(),
                forced: s.disposition.as_ref().is_some_and(|d| d.forced == Some(1)),
            })
            .collect(),
        chapters: raw.chapters.iter().filter_map(build_chapter).collect(),
        unreadable: None,
    }
}

// The cover-art test must stay inside the predicate, or a leading mjpeg poster
// stream wins and nulls out the actual video.
fn video_stream(raw: &FfprobeOutput) -> Option<&FfStream> {
    raw.streams
        .iter()
        .find(|&s| s.codec_type.as_deref() == Some("video") && !is_probably_cover_art(s))
}

pub(super) fn video_index(raw: &FfprobeOutput) -> Option<u32> {
    video_stream(raw).and_then(|s| s.index)
}

fn build_chapter(c: &FfChapter) -> Option<Chapter> {
    let start = c
        .start_time
        .as_deref()
        .and_then(|s| s.parse::<f64>().ok())?;
    let end = c.end_time.as_deref().and_then(|s| s.parse::<f64>().ok())?;
    if end <= start {
        return None;
    }
    Some(Chapter {
        start_ms: (start * 1000.0) as u64,
        end_ms: (end * 1000.0) as u64,
        title: c
            .tags
            .as_ref()
            .and_then(|t| t.title.clone())
            .filter(|t| !t.trim().is_empty()),
    })
}

fn is_probably_cover_art(stream: &FfStream) -> bool {
    matches!(stream.codec_name.as_deref(), Some("mjpeg") | Some("png"))
        && stream.width.unwrap_or(0) <= 1000
        && stream.height.unwrap_or(0) <= 1000
}

fn build_video(stream: &FfStream) -> VideoStream {
    let bit_depth = stream
        .bits_per_raw_sample
        .as_deref()
        .and_then(|s| s.parse::<u32>().ok())
        .or_else(|| pixel_format_bit_depth(stream.pix_fmt.as_deref()));

    let dovi = stream.dovi();
    let hdr_format = hdr_format(stream, bit_depth, dovi.is_some());
    let color = ColorInfo {
        primaries: stream.color_primaries.clone(),
        transfer: stream.color_transfer.clone(),
        matrix: stream.color_space.clone(),
    };

    VideoStream {
        codec: normalize_codec(stream.codec_name.as_deref()),
        width: stream.width,
        height: stream.height,
        hdr: hdr_format.is_some(),
        bit_depth,
        hdr_format,
        dolby_vision_profile: dovi.and_then(|d| d.dv_profile),
        dolby_vision_level: dovi.and_then(|d| d.dv_level),
        dolby_vision_compatibility: dovi.and_then(|d| d.dv_bl_signal_compatibility_id),
        dolby_vision_enhancement_layer: dovi.is_some_and(|d| d.el_present_flag == Some(1)),
        color: (!color.is_empty()).then_some(color),
    }
}

fn build_audio(stream: &FfStream, index: u32) -> AudioStream {
    let (language, title) = (stream.language(), stream.title());
    AudioStream {
        index,
        codec: normalize_codec(stream.codec_name.as_deref()),
        channels: stream.channels,
        region: kroma_primitives::language::track_region(language.as_deref(), title.as_deref()),
        language,
        title,
        default: stream
            .disposition
            .as_ref()
            .is_some_and(|d| d.default == Some(1)),
        spatial: stream.profile.as_deref().and_then(spatial_of),
        profile: stream.profile.clone().filter(|p| !p.is_empty()),
        layout: stream.channel_layout.clone().filter(|l| !l.is_empty()),
    }
}

/// The object layer ffprobe names in a stream's profile: `Dolby TrueHD + Dolby
/// Atmos` and `Dolby Digital Plus + Dolby Atmos` for Atmos, `DTS-HD MA + DTS:X`
/// (and its IMAX variant) for DTS:X.
pub fn spatial_of(profile: &str) -> Option<SpatialAudio> {
    let profile = profile.to_ascii_lowercase();
    if profile.contains("atmos") {
        Some(SpatialAudio::Atmos)
    } else if profile.contains("dts:x") {
        Some(SpatialAudio::Dtsx)
    } else {
        None
    }
}

// The DOVI record outranks the transfer function, because a Dolby Vision stream
// can signal any transfer or none at all. HDR10+ is per-frame SEI that a header
// read cannot see, so it is added afterwards by `run`. Wide gamut at 10 bits only
// reads as HDR10 where the transfer is left unstated: one stated as anything else
// (`bt709`, `bt2020-10`) is an SDR picture in a wide container.
fn hdr_format(stream: &FfStream, bit_depth: Option<u32>, dolby_vision: bool) -> Option<HdrFormat> {
    if dolby_vision {
        return Some(HdrFormat::DolbyVision);
    }
    match stream.color_transfer.as_deref().unwrap_or("") {
        "smpte2084" => return Some(HdrFormat::Hdr10),
        "arib-std-b67" => return Some(HdrFormat::Hlg),
        _ => {}
    }
    let transfer_unstated = matches!(
        stream.color_transfer.as_deref().unwrap_or(""),
        "" | "unknown" | "unspecified"
    );
    let wide_gamut = matches!(stream.color_primaries.as_deref().unwrap_or(""), "bt2020");
    let deep = bit_depth.is_some_and(|b| b >= 10);
    (transfer_unstated && deep && wide_gamut).then_some(HdrFormat::Hdr10)
}

fn pixel_format_bit_depth(pix_fmt: Option<&str>) -> Option<u32> {
    let pix_fmt = pix_fmt?;
    if pix_fmt.contains("p10") || pix_fmt.contains("10le") || pix_fmt.contains("10be") {
        Some(10)
    } else if pix_fmt.contains("p12") || pix_fmt.contains("12le") || pix_fmt.contains("12be") {
        Some(12)
    } else if !pix_fmt.is_empty() {
        Some(8)
    } else {
        None
    }
}

/// The lowercase canonical form clients expect.
pub fn normalize_codec(name: Option<&str>) -> String {
    let raw = name.unwrap_or("unknown").to_ascii_lowercase();
    match raw.as_str() {
        "h265" | "hevc" => "hevc",
        "h264" | "avc" => "h264",
        "av01" | "av1" => "av1",
        "vp09" | "vp9" => "vp9",
        "vp08" | "vp8" => "vp8",
        "mpeg4" => "mpeg4",
        "eac3" | "e-ac-3" => "eac3",
        "ac3" | "ac-3" => "ac3",
        "dca" | "dts" => "dts",
        "truehd" => "truehd",
        "mp4a" | "aac" => "aac",
        "mp3" | "mp3float" => "mp3",
        "flac" => "flac",
        "opus" => "opus",
        "vorbis" => "vorbis",
        "subrip" | "srt" => "subrip",
        "ass" | "ssa" => "ass",
        "hdmv_pgs_subtitle" | "pgs" => "pgs",
        "mov_text" => "mov_text",
        // Hand back the owned, already-lowercased string rather than a copy.
        _ => return raw,
    }
    .to_string()
}

#[cfg(test)]
mod tests;
