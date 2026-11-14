//! HDR10+ is per-frame SEI that the stream header never mentions, so a stream
//! already PQ gets one more bounded ffprobe pass over its first frame. Any
//! failure leaves it the HDR10 it also is.

use std::ffi::OsString;
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use kroma_module_wire::{HdrFormat, VideoStream};

use crate::input::{self, MediaInput};

use super::ffprobe_output::FfFrames;

const BUDGET: Duration = Duration::from_secs(8);
const OUTPUT_CAP: u64 = 1 << 20;
const POLL: Duration = Duration::from_millis(10);

pub(super) fn is_pq_hdr10(video: &VideoStream) -> bool {
    video.hdr_format == Some(HdrFormat::Hdr10)
        && video
            .color
            .as_ref()
            .is_some_and(|c| c.transfer.as_deref() == Some("smpte2084"))
}

pub(super) fn upgrade(video: &mut VideoStream, detect: impl FnOnce() -> bool) {
    if is_pq_hdr10(video) && detect() {
        video.hdr_format = Some(HdrFormat::Hdr10Plus);
    }
}

pub(super) fn first_frame_carries(input: &MediaInput, stream: u32) -> bool {
    let Some(stdout) = run("ffprobe", &args(input, stream), BUDGET) else {
        return false;
    };
    serde_json::from_slice::<FfFrames>(&stdout).is_ok_and(|frames| carries(&frames))
}

pub(super) fn carries(frames: &FfFrames) -> bool {
    frames.frames.iter().any(|frame| {
        frame.side_data_list.iter().any(|side| {
            side.side_data_type.as_deref().is_some_and(|name| {
                let name = name.to_ascii_lowercase();
                name.contains("2094-40") || name.contains("hdr10+")
            })
        })
    })
}

fn args(input: &MediaInput, stream: u32) -> Vec<OsString> {
    let mut args: Vec<OsString> = [
        "-v",
        "error",
        "-select_streams",
        &stream.to_string(),
        "-show_entries",
        "frame_side_data_list",
        "-read_intervals",
        "%+#1",
        "-of",
        "json",
    ]
    .iter()
    .map(OsString::from)
    .collect();
    args.extend(input::media(input));
    args
}

fn run(program: &str, args: &[OsString], budget: Duration) -> Option<Vec<u8>> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut out = Vec::new();
        let _ = stdout.take(OUTPUT_CAP).read_to_end(&mut out);
        out
    });
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return status.success().then(|| reader.join().ok()).flatten();
            }
            Ok(None) if started.elapsed() < budget => std::thread::sleep(POLL),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

#[cfg(test)]
mod tests;
