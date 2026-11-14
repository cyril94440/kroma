// Option sets shared by the mpv binary and the in-process libmpv engines.
// Pure: no tauri, no libmpv, so every vector is unit-testable on any host.

pub const PASSTHROUGH_ENV: &str = "KROMA_AUDIO_SPDIF";

const SPDIF_CODECS: &[&str] = &["ac3", "eac3", "dts", "dts-hd", "truehd"];
const SPDIF_DEFAULT: &str = "ac3,eac3,dts-hd,truehd";
const COLORSPACE_HINT: (&str, &str) = ("target-colorspace-hint", "auto");

/// `gpu-next` first, plain `gpu` when it cannot start. No trailing comma: a
/// list that stops here fails the engine, so the caller can fall back.
pub const IN_PROCESS_VO: &str = "gpu-next,gpu";

/// Codecs to bitstream to a receiver, from the raw `KROMA_AUDIO_SPDIF` value.
/// Unset, empty, `0`, `no`, `off` and `false` mean off; `1`, `yes`, `on`, `true`
/// and `all` mean every codec mpv can pass; anything else is a comma list in
/// which unknown names are dropped. `None` means keep decoding to PCM.
pub fn passthrough_codecs(raw: Option<&str>) -> Option<String> {
    let raw = raw?.trim().to_ascii_lowercase();
    match raw.as_str() {
        "" | "0" | "no" | "off" | "false" => None,
        "1" | "yes" | "on" | "true" | "all" => Some(SPDIF_DEFAULT.to_owned()),
        list => {
            let mut kept: Vec<&str> = Vec::new();
            for name in list.split(',').map(str::trim) {
                if SPDIF_CODECS.contains(&name) && !kept.contains(&name) {
                    kept.push(name);
                }
            }
            (!kept.is_empty()).then(|| kept.join(","))
        }
    }
}

pub fn passthrough_from_env() -> Option<String> {
    passthrough_codecs(std::env::var(PASSTHROUGH_ENV).ok().as_deref())
}

/// Command-line flags for a launched mpv binary, appended after its base flags.
/// Only the pinned sidecar gets the colourspace hint: a system mpv older than
/// 0.39 exits on a flag it does not know, which would sink every ladder rung.
pub fn binary_args(sidecar: bool, spdif: Option<&str>) -> Vec<String> {
    let mut args = Vec::new();
    if sidecar {
        args.push(format!("--{}={}", COLORSPACE_HINT.0, COLORSPACE_HINT.1));
        if let Some(codecs) = spdif {
            args.push(format!("--audio-spdif={codecs}"));
        }
    }
    args
}

/// `set_property` pairs sent over IPC, where an option an old system mpv does
/// not know is an error reply rather than an abort.
pub fn ipc_properties(spdif: Option<&str>) -> Vec<(&'static str, String)> {
    spdif
        .map(|codecs| vec![("audio-spdif", codecs.to_owned())])
        .unwrap_or_default()
}

/// Options a libmpv engine sets after its video output, each one ignorable when
/// this libmpv rejects it. `hdr_hint` is false where the manual lists no
/// supporting context (macOS).
pub fn libmpv_options(hdr_hint: bool, spdif: Option<&str>) -> Vec<(&'static str, String)> {
    let mut options = Vec::new();
    if hdr_hint {
        options.push((COLORSPACE_HINT.0, COLORSPACE_HINT.1.to_owned()));
    }
    if let Some(codecs) = spdif {
        options.push(("audio-spdif", codecs.to_owned()));
    }
    options
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaves_passthrough_off_when_nothing_is_set() {
        assert_eq!(passthrough_codecs(None), None);
        assert_eq!(passthrough_codecs(Some("")), None);
        assert_eq!(passthrough_codecs(Some("0")), None);
        assert_eq!(passthrough_codecs(Some(" Off ")), None);
    }

    #[test]
    fn turns_passthrough_on_for_every_codec_with_a_plain_yes() {
        assert_eq!(
            passthrough_codecs(Some("1")).as_deref(),
            Some("ac3,eac3,dts-hd,truehd")
        );
        assert_eq!(
            passthrough_codecs(Some("TRUE")).as_deref(),
            Some("ac3,eac3,dts-hd,truehd")
        );
    }

    #[test]
    fn keeps_only_codecs_the_manual_lists() {
        assert_eq!(
            passthrough_codecs(Some("ac3, aac ,eac3,ac3")).as_deref(),
            Some("ac3,eac3")
        );
    }

    #[test]
    fn stays_off_when_no_listed_codec_remains() {
        assert_eq!(passthrough_codecs(Some("aac,flac")), None);
    }

    #[test]
    fn gives_the_sidecar_the_hint_and_no_passthrough_by_default() {
        assert_eq!(
            binary_args(true, None),
            vec!["--target-colorspace-hint=auto".to_owned()]
        );
    }

    #[test]
    fn gives_the_sidecar_passthrough_when_asked() {
        assert_eq!(
            binary_args(true, Some("ac3,eac3")),
            vec![
                "--target-colorspace-hint=auto".to_owned(),
                "--audio-spdif=ac3,eac3".to_owned()
            ]
        );
    }

    #[test]
    fn gives_a_system_mpv_no_command_line_flag() {
        assert!(binary_args(false, Some("ac3")).is_empty());
    }

    #[test]
    fn sends_passthrough_over_ipc_only_when_asked() {
        assert_eq!(
            ipc_properties(Some("ac3")),
            vec![("audio-spdif", "ac3".to_owned())]
        );
        assert!(ipc_properties(None).is_empty());
    }

    #[test]
    fn sets_the_hint_on_windows_and_linux_libmpv() {
        assert_eq!(
            libmpv_options(true, None),
            vec![("target-colorspace-hint", "auto".to_owned())]
        );
    }

    #[test]
    fn sets_no_hint_on_macos_libmpv() {
        assert!(libmpv_options(false, None).is_empty());
    }

    #[test]
    fn adds_passthrough_to_libmpv_after_the_hint() {
        assert_eq!(
            libmpv_options(true, Some("truehd")),
            vec![
                ("target-colorspace-hint", "auto".to_owned()),
                ("audio-spdif", "truehd".to_owned())
            ]
        );
    }

    #[test]
    fn lists_gpu_next_before_gpu_without_a_catch_all() {
        assert_eq!(IN_PROCESS_VO, "gpu-next,gpu");
        assert!(!IN_PROCESS_VO.ends_with(','));
    }
}
