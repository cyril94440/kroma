//! What ffprobe's answer means: a read, a refusal worth recording, a failure to
//! fetch that a later pass retries, or no answer at all.

use tracing::debug;

use crate::input::MediaInput;

use super::parse::{build_result, video_index};
use super::ProbeResult;

pub(super) const REASON_CHARS: usize = 200;
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

pub(super) enum Outcome {
    Read(Box<ProbeResult>, Option<u32>),
    Unreadable(String),
    NotProbed,
    Killed,
    Network,
}

pub(super) fn label(input: &MediaInput) -> String {
    match input {
        MediaInput::Local(path) => path.display().to_string(),
        MediaInput::Remote(_) => "the relay".to_string(),
    }
}

pub(super) fn outcome_of(
    input: &MediaInput,
    exit_code: Option<i32>,
    stdout: &[u8],
    stderr: &[u8],
) -> Outcome {
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

pub(super) fn names_a_network_failure(stderr: &[u8]) -> bool {
    let text = String::from_utf8_lossy(stderr).to_ascii_lowercase();
    NETWORK_SIGNS.iter().any(|sign| text.contains(sign))
}

pub(super) fn failure_reason(input: &MediaInput, stderr: &[u8], exit_code: i32) -> String {
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
