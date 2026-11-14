//! Invoke the `ffprobe` CLI on one file: availability check, the per-file run,
//! and what a refusal means against an extension guess.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use tracing::debug;

use kroma_module_wire::VideoStream;

use crate::input::{self, MediaInput};

use super::hdr10plus;
use super::parse::{build_result, video_index};
use super::ProbeResult;

const REASON_CHARS: usize = 200;
const REMOTE_BUDGET: Duration = Duration::from_secs(120);
const OUTPUT_CAP: u64 = 16 * 1024 * 1024;
const POLL: Duration = Duration::from_millis(50);
const NETWORK_SIGNS: [&str; 12] = [
    "server returned",
    "http error",
    "i/o error",
    "input/output error",
    "connection",
    "timed out",
    "timeout",
    "end of file",
    "network",
    "broken pipe",
    "temporarily unavailable",
    "error number -",
];

pub fn ffprobe_available() -> bool {
    Command::new("ffprobe")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub(super) const DURATION_ARGS: [&str; 6] = [
    "-v",
    "error",
    "-show_entries",
    "format=duration",
    "-of",
    "default=nw=1:nk=1",
];

/// Reads just the container's `format.duration`: a header read, no frame decode,
/// so it is cheap enough to call on demand. `None` when ffprobe is missing or the
/// file has no readable duration.
pub fn probe_duration_ms(input: &MediaInput) -> Option<u64> {
    let mut command = Command::new("ffprobe");
    input::without_proxies(&mut command);
    command.args(DURATION_ARGS).args(input::media(input));
    let out = run_bounded(command, budget_for(input))?;
    if !out.status.success() {
        return None;
    }
    super::duration_of(&out.stdout)
}

/// Best effort in one direction only: a missing ffprobe or output KROMA cannot
/// parse falls back to a container-extension guess, because neither is the
/// file's fault. A file ffprobe opened and refused comes back carrying its
/// reason and no invented streams.
///
/// `None` where ffprobe was killed before it answered: nothing was learned about
/// the file, so the caller must leave it to a later pass rather than record a
/// verdict that would stand for good.
pub fn probe_file(path: &Path, ffprobe_present: bool) -> Option<ProbeResult> {
    let container = path.extension().and_then(std::ffi::OsStr::to_str).unwrap_or("");
    probe_input(&MediaInput::Local(path.to_path_buf()), container, ffprobe_present)
}

/// [`probe_file`] over any input. `container` is the real container's extension,
/// which the extension guess falls back on when ffprobe cannot answer: a
/// network URL carries none.
pub fn probe_input(
    input: &MediaInput,
    container: &str,
    ffprobe_present: bool,
) -> Option<ProbeResult> {
    match probe_outcome(input, container, ffprobe_present) {
        Probed::Done(result) => Some(result),
        Probed::Killed | Probed::Network => None,
    }
}

/// What probing one input came to. `Killed` and `Network` learned nothing about
/// the file, so neither may stand as a verdict: a file whose bytes could not be
/// fetched is not an unreadable file.
#[derive(Debug)]
pub enum Probed {
    Done(ProbeResult),
    Killed,
    Network,
}

pub fn probe_outcome(input: &MediaInput, container: &str, ffprobe_present: bool) -> Probed {
    if !ffprobe_present {
        return Probed::Done(fallback_from_container(container));
    }
    match run_ffprobe(input) {
        Outcome::Read(result, video_stream) => {
            Probed::Done(with_hdr10plus(input, *result, video_stream))
        }
        Outcome::Unreadable(reason) => Probed::Done(ProbeResult {
            unreadable: Some(reason),
            ..ProbeResult::default()
        }),
        Outcome::NotProbed => Probed::Done(fallback_from_container(container)),
        Outcome::Killed => Probed::Killed,
        Outcome::Network => Probed::Network,
    }
}

fn budget_for(input: &MediaInput) -> Option<Duration> {
    input.local().is_none().then_some(REMOTE_BUDGET)
}

/// Runs `command` to the end, or kills it once `budget` has passed. `None` when
/// it could not be spawned or was killed for time.
fn run_bounded(mut command: Command, budget: Option<Duration>) -> Option<Output> {
    input::without_proxies(&mut command);
    let Some(budget) = budget else {
        return command.stdin(Stdio::null()).output().ok();
    };
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let (mut stdout, mut stderr) = (child.stdout.take()?, child.stderr.take()?);
    let out_reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = (&mut stdout).take(OUTPUT_CAP).read_to_end(&mut bytes);
        bytes
    });
    let err_reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = (&mut stderr).take(OUTPUT_CAP).read_to_end(&mut bytes);
        bytes
    });
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < budget => std::thread::sleep(POLL),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    };
    Some(Output {
        status,
        stdout: out_reader.join().unwrap_or_default(),
        stderr: err_reader.join().unwrap_or_default(),
    })
}

enum Outcome {
    Read(Box<ProbeResult>, Option<u32>),
    Unreadable(String),
    NotProbed,
    Killed,
    Network,
}

fn with_hdr10plus(input: &MediaInput, mut result: ProbeResult, stream: Option<u32>) -> ProbeResult {
    if let (Some(video), Some(stream)) = (result.video.as_mut(), stream) {
        hdr10plus::upgrade(video, || hdr10plus::first_frame_carries(input, stream));
    }
    result
}

// Failures log at DEBUG, not WARN: every worker probing every file would flood
// the default log, and a wholesale degradation still shows up as `probed` ≪
// `total` in the phase-2 summary.
fn run_ffprobe(input: &MediaInput) -> Outcome {
    let mut command = Command::new("ffprobe");
    input::without_proxies(&mut command);
    command
        .args(["-v", "error", "-show_format", "-show_streams", "-show_chapters", "-of", "json"])
        .args(input::media(input));
    let Some(output) = run_bounded(command, budget_for(input)) else {
        return if input.local().is_some() {
            debug!(file = %label(input), "ffprobe failed to spawn; using extension guess");
            Outcome::NotProbed
        } else {
            debug!(file = %label(input), "ffprobe could not read the title in time");
            Outcome::Network
        };
    };
    outcome_of(input, output.status.code(), &output.stdout, &output.stderr)
}

fn label(input: &MediaInput) -> String {
    match input {
        MediaInput::Local(path) => path.display().to_string(),
        MediaInput::Remote(_) => "the relay".to_string(),
    }
}

fn outcome_of(input: &MediaInput, exit_code: Option<i32>, stdout: &[u8], stderr: &[u8]) -> Outcome {
    let Some(code) = exit_code else {
        debug!(file = %label(input), "ffprobe was killed before it answered");
        return Outcome::Killed;
    };
    if code != 0 {
        if input.local().is_none() && names_a_network_failure(stderr) {
            debug!(file = %label(input), code, "ffprobe could not fetch the title");
            return Outcome::Network;
        }
        let reason = failure_reason(input, stderr, code);
        debug!(file = %label(input), code, %reason, "ffprobe refused the file");
        return Outcome::Unreadable(reason);
    }
    match serde_json::from_slice(stdout) {
        Ok(parsed) => {
            let video_stream = video_index(&parsed);
            Outcome::Read(Box::new(build_result(parsed)), video_stream)
        }
        Err(e) => {
            debug!(file = %label(input), error = %e, "failed to parse ffprobe JSON; using extension guess");
            Outcome::NotProbed
        }
    }
}

fn names_a_network_failure(stderr: &[u8]) -> bool {
    let text = String::from_utf8_lossy(stderr).to_ascii_lowercase();
    NETWORK_SIGNS.iter().any(|sign| text.contains(sign))
}

fn failure_reason(input: &MediaInput, stderr: &[u8], exit_code: i32) -> String {
    let text = String::from_utf8_lossy(stderr);
    // The reason reaches clients, which are never told a file's absolute path
    // nor the bearer URL that stands for it.
    let said = text
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| match input {
            MediaInput::Local(path) => line
                .strip_prefix(format!("{}: ", path.display()).as_str())
                .unwrap_or(line)
                .to_string(),
            MediaInput::Remote(relayed) => relayed.redact(line).replace("the relay: ", ""),
        })
        .unwrap_or_default();
    if said.is_empty() {
        return format!("ffprobe exited with status {exit_code}");
    }
    said.chars().take(REASON_CHARS).collect()
}

fn fallback_from_container(container: &str) -> ProbeResult {
    let ext = container.to_ascii_lowercase();

    let codec = match ext.as_str() {
        "webm" => "vp9",
        "avi" => "mpeg4",
        "mp4" | "m4v" | "mov" | "mkv" | "ts" => "h264",
        _ => "unknown",
    };

    ProbeResult {
        video: Some(VideoStream {
            codec: codec.to_string(),
            width: None,
            height: None,
            hdr: false,
            bit_depth: None,
            hdr_format: None,
            dolby_vision_profile: None,
            dolby_vision_level: None,
            dolby_vision_compatibility: None,
            dolby_vision_enhancement_layer: false,
            color: None,
        }),
        ..ProbeResult::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reason_of(outcome: Outcome) -> String {
        match outcome {
            Outcome::Unreadable(reason) => reason,
            Outcome::Read(..) => panic!("a refusal must not read as a description"),
            Outcome::NotProbed => panic!("a refusal must not read as a missing probe"),
            Outcome::Killed | Outcome::Network => panic!("a refusal must not read as a killed probe"),
        }
    }

    #[test]
    fn a_file_ffprobe_refuses_is_unreadable_and_keeps_ffprobes_own_reason() {
        let path = MediaInput::Local("/media/Dune (2021)/dune.mkv".into());
        let stderr = b"/media/Dune (2021)/dune.mkv: Invalid data found when processing input\n";

        let reason = reason_of(outcome_of(&path, Some(1), b"", stderr));

        assert_eq!(reason, "Invalid data found when processing input");
    }

    #[test]
    fn the_reason_never_carries_the_path_the_client_is_not_shown() {
        let path = MediaInput::Local("/srv/media/private/x.mkv".into());
        let stderr = b"/srv/media/private/x.mkv: moov atom not found\n";

        let reason = reason_of(outcome_of(&path, Some(1), b"", stderr));

        assert!(!reason.contains("/srv/media"), "{reason}");
    }

    #[test]
    fn a_refusal_with_nothing_to_say_still_names_the_status_it_failed_with() {
        let path = MediaInput::Local("/media/x.mkv".into());

        assert_eq!(
            reason_of(outcome_of(&path, Some(183), b"", b"  \n")),
            "ffprobe exited with status 183"
        );
    }

    #[test]
    fn a_probe_a_signal_took_is_no_verdict_about_the_file() {
        let outcome = outcome_of(&MediaInput::Local("/media/x.mkv".into()), None, b"", b"");

        assert!(matches!(outcome, Outcome::Killed));
    }

    #[test]
    fn a_reason_longer_than_the_cap_is_cut_to_it() {
        let path = MediaInput::Local("/media/x.mkv".into());
        let stderr = "e".repeat(REASON_CHARS * 2).into_bytes();

        let reason = reason_of(outcome_of(&path, Some(1), b"", &stderr));

        assert_eq!(reason.chars().count(), REASON_CHARS);
    }

    #[test]
    fn output_kroma_cannot_parse_is_not_the_files_fault() {
        let outcome = outcome_of(&MediaInput::Local("/media/x.mkv".into()), Some(0), b"not json", b"");

        assert!(matches!(outcome, Outcome::NotProbed));
    }

    #[test]
    fn a_described_file_comes_back_with_its_streams_and_no_fault() {
        let stdout = br#"{"streams":[{"codec_type":"video","codec_name":"hevc","width":3840,"height":2160}]}"#;

        let outcome = outcome_of(&MediaInput::Local("/media/x.mkv".into()), Some(0), stdout, b"");

        let Outcome::Read(result, _) = outcome else {
            panic!("a parsable description must read as one");
        };
        assert_eq!(
            result.video.as_ref().map(|v| v.codec.as_str()),
            Some("hevc")
        );
        assert!(result.unreadable.is_none());
    }

    #[test]
    fn without_ffprobe_the_container_extension_still_names_a_codec() {
        let guessed = probe_file(Path::new("/media/x.webm"), false).expect("a guess");

        assert_eq!(guessed.video.map(|v| v.codec), Some("vp9".to_string()));
        assert!(guessed.unreadable.is_none());
    }

    #[test]
    fn a_container_nothing_recognises_guesses_nothing() {
        let guessed = probe_file(Path::new("/media/x.bin"), false).expect("a guess");

        assert_eq!(guessed.video.map(|v| v.codec), Some("unknown".to_string()));
    }
}
