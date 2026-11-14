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
use super::outcome::{label, outcome_of, Outcome};
use super::ProbeResult;

const REMOTE_BUDGET: Duration = Duration::from_secs(120);
const OUTPUT_CAP: u64 = 16 * 1024 * 1024;
const POLL: Duration = Duration::from_millis(50);
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
    let container = path
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or("");
    probe_input(
        &MediaInput::Local(path.to_path_buf()),
        container,
        ffprobe_present,
    )
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
        Probed::Done(result) => Some(*result),
        Probed::Killed | Probed::Network => None,
    }
}

/// What probing one input came to. `Killed` and `Network` learned nothing about
/// the file, so neither may stand as a verdict: a file whose bytes could not be
/// fetched is not an unreadable file.
#[derive(Debug)]
pub enum Probed {
    Done(Box<ProbeResult>),
    Killed,
    Network,
}

pub fn probe_outcome(input: &MediaInput, container: &str, ffprobe_present: bool) -> Probed {
    if !ffprobe_present {
        return Probed::Done(Box::new(fallback_from_container(container)));
    }
    match run_ffprobe(input) {
        Outcome::Read(result, video_stream) => {
            Probed::Done(Box::new(with_hdr10plus(input, *result, video_stream)))
        }
        Outcome::Unreadable(reason) => Probed::Done(Box::new(ProbeResult {
            unreadable: Some(reason),
            ..ProbeResult::default()
        })),
        Outcome::NotProbed => Probed::Done(Box::new(fallback_from_container(container))),
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
        .args([
            "-v",
            "error",
            "-show_format",
            "-show_streams",
            "-show_chapters",
            "-of",
            "json",
        ])
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
mod tests;
