//! The ffmpeg command line of one HLS session, built from what the catalog
//! knows of the source and the mode the client's decoders settled on.

use std::ffi::OsString;
use std::path::Path;

use kroma_module_wire::{AudioStream, VideoStream};

use crate::{input, stream_copy};

use super::ffmpeg::{keyframe_cadence, SEGMENT_PATTERN, SEGMENT_SECONDS};
use super::hwaccel::Pipeline;
use super::mode::SURROUND_MIN_CHANNELS;
use super::{AudioMode, StreamMode, VideoMode};

// Multiple of realtime ffmpeg may read at, so concurrent sessions don't
// thrash the mount; above 1.0 so clients can still build a forward buffer.
const READRATE: &str = "2.0";
// Seconds read at full speed before READRATE throttling starts (ffmpeg >= 6.1).
const READRATE_BURST: &str = "60";
const SURROUND_BITRATE: &str = "640k";
const SURROUND_CHANNELS: &str = "6";

pub(super) struct Spec<'a> {
    pub input: &'a Path,
    pub dir: &'a Path,
    pub audio: u32,
    pub mode: StreamMode,
    pub start_secs: f64,
    pub burst: bool,
    pub pipeline: Pipeline,
    pub video: Option<&'a VideoStream>,
    pub track: Option<&'a AudioStream>,
}

pub(super) fn build(spec: &Spec) -> Vec<OsString> {
    let mut args: Vec<OsString> = Vec::new();
    let mut push = |items: &[&str]| args.extend(items.iter().map(OsString::from));
    let seeking = spec.start_secs > 0.5;
    let copying_video = matches!(spec.mode.video, VideoMode::Copy);
    push(&["-v", "error", "-nostdin"]);
    // `-threads` lands on the DECODER here (before `-i`): a remux never decodes
    // video, so a pool is pure overhead - but a transcode does, and one thread
    // cannot keep a 4K source ahead of the player.
    if copying_video {
        push(&["-threads", "1"]);
    } else {
        // Puts the decode on the device too where there is one, so a 4K source
        // never touches the CPU on its way to a 1080p screen.
        let decode = spec.pipeline.input_args();
        push(&decode.iter().map(String::as_str).collect::<Vec<_>>());
    }
    if seeking {
        // Required for A/V sync: an accurate seek backs the video to a keyframe but
        // decodes-and-discards audio to the exact `-ss`, starting it a GOP late.
        push(&["-noaccurate_seek", "-ss", &format!("{:.3}", spec.start_secs)]);
    }
    push(&["-readrate", READRATE]);
    if spec.burst {
        push(&["-readrate_initial_burst", READRATE_BURST]);
    }
    args.extend(input::file(spec.input));
    let mut push = |items: &[&str]| args.extend(items.iter().map(OsString::from));
    if seeking {
        push(&["-copyts"]);
    }
    push(&["-map", "0:v:0", "-map", &format!("0:a:{}", spec.audio)]);
    if copying_video {
        push(&["-c:v", "copy"]);
        push(&stream_copy::video_args(spec.video));
    } else {
        if let Some(filter) = spec.pipeline.video_filter(spec.mode.video.box_size()) {
            push(&["-vf", &filter]);
        }
        push(&spec.pipeline.accel.encoder_args(spec.pipeline.effort));
        push(spec.pipeline.output_tags());
        push(&["-force_key_frames", &keyframe_cadence()]);
    }
    push(&audio_args(spec.mode.audio, spec.track));
    let audio_copied = !spec.mode.audio.transcode();
    let strictness = stream_copy::strictness(
        spec.video.filter(|_| copying_video),
        spec.track.map(|t| t.codec.as_str()).filter(|_| audio_copied),
    );
    if let Some(level) = strictness {
        push(&["-strict", level]);
    }
    let hls_time = SEGMENT_SECONDS.to_string();
    let progress = spec.dir.join(super::progress::FILE);
    args.extend(
        [
            "-progress".into(),
            progress.into_os_string(),
            "-stats_period".into(),
            "1".into(),
            "-f".into(),
            "hls".into(),
            "-hls_time".into(),
            hls_time.into(),
            "-hls_playlist_type".into(),
            "event".into(),
            "-hls_segment_type".into(),
            "fmp4".into(),
            "-hls_fmp4_init_filename".into(),
            "init.mp4".into(),
            "-hls_segment_filename".into(),
            spec.dir.join(SEGMENT_PATTERN).into_os_string(),
            "-hls_flags".into(),
            "independent_segments+temp_file".into(),
            spec.dir.join("index.m3u8").into_os_string(),
        ],
    );
    args
}

pub(super) fn audio_args(mode: AudioMode, track: Option<&AudioStream>) -> Vec<&'static str> {
    if !mode.transcode() {
        return vec!["-c:a", "copy"];
    }
    let mut args = Vec::new();
    if let Some(filter) = mode.filter_chain() {
        args.extend(["-af", filter]);
    }
    let surround = track
        .and_then(|t| t.channels)
        .is_some_and(|channels| channels >= SURROUND_MIN_CHANNELS);
    let codec = match mode {
        AudioMode::Eac3 if surround => Some("eac3"),
        AudioMode::Ac3 if surround => Some("ac3"),
        _ => None,
    };
    match codec {
        Some(codec) => args.extend([
            "-c:a",
            codec,
            "-ac",
            SURROUND_CHANNELS,
            "-b:a",
            SURROUND_BITRATE,
        ]),
        None => args.extend(["-c:a", "aac", "-ac", "2", "-b:a", "192k"]),
    }
    args
}

#[cfg(test)]
mod tests;
