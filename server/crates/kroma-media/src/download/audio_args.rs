//! Which audio tracks the download stream-copies and which it re-encodes,
//! as the ffmpeg flags that say so.

use kroma_module_wire::AudioStream;

use crate::hls::SURROUND_MIN_CHANNELS;

// Copy-safe means fMP4-muxable (mirrors the clients' `FMP4_COPY_CODECS`) AND, when
// the client declared a `copy` set, present in it - Android phones usually lack
// Dolby decoders and send `aac`, iOS sends `aac,ac3,eac3`.
fn download_copies_audio(codec: Option<&str>, client_set: Option<&str>) -> bool {
    let Some(codec) = codec else { return false };
    if !matches!(codec, "aac" | "ac3" | "eac3") {
        return false;
    }
    match client_set {
        None => true,
        Some(set) => set.split(',').any(|c| c.trim().eq_ignore_ascii_case(codec)),
    }
}

enum Treatment {
    Copy,
    Surround(&'static str),
    Stereo,
}

fn treatment(track: Option<&AudioStream>, fallback: Option<&str>, set: Option<&str>) -> Treatment {
    let codec = track.map(|t| t.codec.as_str()).or(fallback);
    if download_copies_audio(codec, set) {
        return Treatment::Copy;
    }
    let surround = track
        .and_then(|t| t.channels)
        .is_some_and(|channels| channels >= SURROUND_MIN_CHANNELS);
    let declares = |name: &str| {
        set.is_some_and(|set| set.split(',').any(|c| c.trim().eq_ignore_ascii_case(name)))
    };
    surround_target(surround, declares("eac3"), declares("ac3"), crate::capabilities::audio_targets())
}

fn surround_target(
    surround: bool,
    eac3: bool,
    ac3: bool,
    encoders: crate::capabilities::AudioTargets,
) -> Treatment {
    match (surround, eac3 && encoders.eac3, ac3 && encoders.ac3) {
        (true, true, _) => Treatment::Surround("eac3"),
        (true, false, true) => Treatment::Surround("ac3"),
        _ => Treatment::Stereo,
    }
}

// Every track rides along in stream order: AVFoundation synthesizes local audio
// selection from the MP4 muxer's single alternate group, so offline pickers select
// by ordinal.
pub(super) fn download_audio_args(
    tracks: &[AudioStream],
    fallback_codec: Option<&str>,
    client_set: Option<&str>,
) -> Vec<String> {
    if tracks.is_empty() {
        // Unprobed item: the map stays optional so a video-only file still downloads.
        let mut args: Vec<String> = vec!["-map".into(), "0:a:0?".into()];
        args.extend(audio_codec_args(":a", treatment(None, fallback_codec, client_set)));
        return args;
    }
    let mut sorted: Vec<&AudioStream> = tracks.iter().collect();
    sorted.sort_by_key(|t| t.index);
    let mut args: Vec<String> = Vec::new();
    for (out, t) in sorted.iter().enumerate() {
        // NOT optional (`0:a:N?`): the per-track codec options are numbered by OUTPUT
        // position, so a map matching nothing shifts every later stream and lands
        // `copy` on a track meant to be transcoded. Fail loudly instead.
        args.extend(["-map".into(), format!("0:a:{}", t.index)]);
        args.extend(audio_codec_args(
            &format!(":a:{out}"),
            treatment(Some(t), None, client_set),
        ));
    }
    args
}

fn audio_codec_args(spec: &str, treatment: Treatment) -> Vec<String> {
    let (codec, channels, bitrate) = match treatment {
        Treatment::Copy => return vec![format!("-c{spec}"), "copy".into()],
        Treatment::Surround(codec) => (codec, "6", "640k"),
        Treatment::Stereo => ("aac", "2", "192k"),
    };
    vec![
        format!("-c{spec}"),
        codec.into(),
        format!("-ac{spec}"),
        channels.into(),
        format!("-b{spec}"),
        bitrate.into(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_build_without_the_dolby_encoders_keeps_the_stereo_aac_it_always_had() {
        use crate::capabilities::AudioTargets;
        let none = AudioTargets { ac3: false, eac3: false };
        let all = AudioTargets { ac3: true, eac3: true };

        assert!(matches!(surround_target(true, true, true, all), Treatment::Surround("eac3")));
        assert!(matches!(surround_target(true, true, true, none), Treatment::Stereo));
        assert!(matches!(surround_target(true, false, true, AudioTargets { ac3: true, eac3: false }), Treatment::Surround("ac3")));
    }

    fn track(index: u32, codec: &str) -> AudioStream {
        AudioStream {
            index,
            codec: codec.into(),
            channels: None,
            language: None,
            title: None,
            default: false,
            region: None,
            profile: None,
            layout: None,
            spatial: None,
        }
    }

    #[test]
    fn download_copy_gate_honors_safe_set_and_client_set() {
        assert!(download_copies_audio(Some("eac3"), None));
        assert!(!download_copies_audio(Some("dts"), None));
        assert!(!download_copies_audio(None, None));
        assert!(download_copies_audio(Some("aac"), Some("aac")));
        assert!(!download_copies_audio(Some("eac3"), Some("aac")));
        assert!(download_copies_audio(Some("ac3"), Some("aac, AC3")));
        // `?copy=` (present, empty) means the device decodes none of them.
        assert!(!download_copies_audio(Some("aac"), Some("")));
        assert!(!download_copies_audio(Some("eac3"), Some("")));
    }

    #[test]
    fn download_audio_args_unprobed_fallback() {
        assert_eq!(
            download_audio_args(&[], Some("aac"), None),
            ["-map", "0:a:0?", "-c:a", "copy"]
        );
        assert_eq!(
            download_audio_args(&[], Some("dts"), None),
            ["-map", "0:a:0?", "-c:a", "aac", "-ac:a", "2", "-b:a", "192k"]
        );
    }

    #[test]
    fn download_audio_args_maps_every_track_with_per_track_codecs() {
        let tracks = [track(0, "eac3"), track(1, "dts"), track(2, "aac")];
        assert_eq!(
            download_audio_args(&tracks, Some("eac3"), None),
            [
                "-map", "0:a:0", "-c:a:0", "copy", "-map", "0:a:1", "-c:a:1", "aac", "-ac:a:1",
                "2", "-b:a:1", "192k", "-map", "0:a:2", "-c:a:2", "copy",
            ]
        );
    }

    #[test]
    fn download_audio_args_client_set_forces_transcode_and_sorts_by_index() {
        let tracks = [track(1, "aac"), track(0, "eac3")];
        assert_eq!(
            download_audio_args(&tracks, None, Some("aac")),
            [
                "-map", "0:a:0", "-c:a:0", "aac", "-ac:a:0", "2", "-b:a:0", "192k", "-map",
                "0:a:1", "-c:a:1", "copy",
            ]
        );
    }

    fn surround_track(index: u32, codec: &str, channels: u32) -> AudioStream {
        AudioStream {
            channels: Some(channels),
            ..track(index, codec)
        }
    }

    #[test]
    fn a_surround_track_the_client_cannot_copy_keeps_its_bed_when_it_declares_eac3() {
        let tracks = [surround_track(0, "dts", 6)];

        assert_eq!(
            download_audio_args(&tracks, None, Some("aac,eac3")),
            ["-map", "0:a:0", "-c:a:0", "eac3", "-ac:a:0", "6", "-b:a:0", "640k"]
        );
    }

    #[test]
    fn it_falls_back_on_ac3_and_then_on_stereo_aac() {
        let tracks = [surround_track(0, "truehd", 8)];

        assert_eq!(
            download_audio_args(&tracks, None, Some("aac,ac3"))[2..4],
            ["-c:a:0", "ac3"]
        );
        assert_eq!(
            download_audio_args(&tracks, None, Some("aac"))[2..4],
            ["-c:a:0", "aac"]
        );
    }

    #[test]
    fn a_client_that_declared_nothing_still_gets_the_stereo_aac_it_always_got() {
        let tracks = [surround_track(0, "dts", 6)];

        assert_eq!(
            download_audio_args(&tracks, None, None),
            ["-map", "0:a:0", "-c:a:0", "aac", "-ac:a:0", "2", "-b:a:0", "192k"]
        );
    }

    #[test]
    fn a_stereo_track_is_never_upmixed_into_surround() {
        let tracks = [surround_track(0, "flac", 2)];

        assert_eq!(
            download_audio_args(&tracks, None, Some("aac,eac3"))[2..4],
            ["-c:a:0", "aac"]
        );
    }
}
