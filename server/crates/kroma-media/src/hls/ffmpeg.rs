//! The remux process: what is spawned, and the one probe that decides where it
//! starts. Everything here shells out; nothing here touches the session map.

use std::process::Stdio;

#[cfg(test)]
use std::path::PathBuf;

use tokio::process::{Child, Command};

use crate::input::{self, MediaInput};

use super::args::{build, Spec};
use super::naming::contains;

pub(super) const SEGMENT_SECONDS: u32 = 6;
/// The write pattern handed to `-hls_segment_filename`; `naming::seg_index`
/// parses what it produces.
pub const SEGMENT_PATTERN: &str = "seg_%05d.m4s";
/// A keyframe every segment, counted from the first frame OUT rather than from
/// the source clock: a seek runs under `-copyts`, so an expression in absolute
/// time would force a keyframe on every frame until `n_forced * SEGMENT_SECONDS`
/// caught up with the anchor.
///
/// Left alone, the encoder keys on its own schedule (~10 s at libx264's default
/// keyint) and the HLS muxer can only cut where it finds one, so segments run
/// half again as long as the playlist asks for and the first one costs that much
/// media before anything can be played.
pub(super) fn keyframe_cadence() -> String {
    format!("expr:if(isnan(prev_forced_t),1,gte(t,prev_forced_t+{SEGMENT_SECONDS}))")
}

/// What `ffmpeg <args>` printed, both streams together because which one carries
/// the answer varies by build. Empty where ffmpeg could not be run at all.
pub(super) fn ffmpeg_output(args: &[&str]) -> Vec<u8> {
    std::process::Command::new("ffmpeg")
        .args(args)
        .output()
        .map(|o| {
            let mut s = o.stdout;
            s.extend_from_slice(&o.stderr);
            s
        })
        .unwrap_or_default()
}

/// `-readrate_initial_burst` only exists from ffmpeg 6.1; older builds get a
/// plain `-readrate` and a slightly slower first segment.
pub fn detect_burst() -> bool {
    contains(
        &ffmpeg_output(&["-hide_banner", "-h", "full"]),
        b"readrate_initial_burst",
    )
}

/// The keyframe at-or-before `anchor`, which is where the remux really starts
/// and what the client uses as `baseSec`. Falls back to `anchor` itself where
/// the container answers nothing.
///
/// Read as one frame FROM the anchor, not a scan of the window before it: the
/// demuxer's seek is the seek `-ss` will make, so the first frame handed back is
/// the answer, and a scan would demux every byte of that window instead.
pub async fn keyframe_before(input: &MediaInput, anchor: f64) -> f64 {
    if anchor <= 0.5 {
        return 0.0;
    }
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-skip_frame",
            "nokey",
        ])
        .arg("-read_intervals")
        .arg(format!("{anchor:.3}%+#1"))
        .args(["-show_entries", "frame=pts_time", "-of", "csv=p=0"])
        .args(input::media(input))
        .output()
        .await;
    let Ok(out) = out else {
        return anchor;
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.trim().trim_end_matches(',').parse::<f64>().ok())
        .filter(|t| *t <= anchor + 0.01)
        .fold(None, |best: Option<f64>, t| {
            Some(best.map_or(t, |b| b.max(t)))
        })
        .unwrap_or(anchor)
}

// The selected audio track is MUXED into one media playlist rather than exposed as
// an alternate rendition: hls.js keeps playing rendition 0 regardless of selection.
// A language switch is therefore a reload with a different `audio` (a new session).
pub(super) fn spawn_stream(spec: &Spec) -> std::io::Result<Child> {
    let mut cmd = Command::new("ffmpeg");
    cmd.args(build(spec))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .kill_on_drop(true);
    let log = spec
        .input
        .local()
        .and_then(|_| std::fs::File::create(spec.dir.join("ffmpeg.log")).ok());
    match log {
        Some(f) => cmd.stderr(Stdio::from(f)),
        None => cmd.stderr(Stdio::null()),
    };
    cmd.spawn()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(dir: &std::path::Path) -> Option<PathBuf> {
        let path = dir.join("clip.mp4");
        std::process::Command::new("ffmpeg")
            .args(["-v", "error"])
            .args(input::generated("testsrc=duration=20:size=64x48:rate=25"))
            .args(["-c:v", "libx264", "-g", "50", "-y"])
            .arg(&path)
            .status()
            .ok()?
            .success()
            .then_some(path)
    }

    #[tokio::test]
    async fn a_seek_starts_on_the_keyframe_at_or_before_the_second_asked_for() {
        let dir = kroma_testing::temp_dir("hls-keyframe");
        let Some(path) = clip(dir.path()) else {
            return;
        };

        let start = keyframe_before(&MediaInput::Local(path.clone()), 11.0).await;

        assert!((start - 10.0).abs() < 0.05, "{start}");
    }

    #[tokio::test]
    async fn a_second_the_container_cannot_answer_for_keeps_the_anchor() {
        let start = keyframe_before(&MediaInput::Local("http://127.0.0.1:1/clip.mp4".into()), 42.0).await;

        assert_eq!(start, 42.0);
    }

    #[test]
    fn the_keyframe_cadence_is_measured_from_the_first_frame_out() {
        let expr = keyframe_cadence();

        assert!(expr.contains("prev_forced_t"), "{expr}");
        assert!(expr.contains("isnan"), "{expr}");
        // Never `n_forced * 6`: under `-copyts` a seek starts the output at the
        // anchor, and an expression in absolute time forces a keyframe on every
        // frame until the count has caught up with it.
        assert!(!expr.contains("n_forced"), "{expr}");
    }

    #[test]
    fn a_keyframe_lands_on_every_segment_boundary() {
        assert!(keyframe_cadence().contains(&format!("+{SEGMENT_SECONDS})")));
    }
}
