use std::sync::LazyLock;

use regex::Regex;

pub const SAMPLE_SECONDS: f64 = 6.0;
pub const FULL_SECONDS: f64 = 4.0;
pub const MIN_KEY_FRAMES: usize = 3;
pub const WINDOW_SHARE: f64 = 0.5;
pub const FREEZE_MIN_SECONDS: f64 = 2.0;
pub const BLACK_CONFIRM: u32 = 2;
pub const FROZEN_CONFIRM: u32 = 2;

static FRAMES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"frame=\s*(\d+)").unwrap());
static TIME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"time=(\d+):(\d+):(\d+\.\d+)").unwrap());
static BLACK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"black_duration:([\d.]+)").unwrap());
static FREEZE_START: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"freeze_start:\s*([\d.]+)").unwrap());
static FREEZE_DURATION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"freeze_duration:\s*([\d.]+)").unwrap());

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Verdict {
    pub black: bool,
    pub frozen: bool,
}

pub fn probe_args(source: &str, full: bool) -> Vec<String> {
    let window = if full { FULL_SECONDS } else { SAMPLE_SECONDS };
    let mut args: Vec<String> = ["-v", "info", "-fflags", "nobuffer", "-threads", "1"].map(String::from).to_vec();
    if !full {
        args.extend(["-skip_frame", "nokey"].map(String::from));
    }
    args.extend(
        [
            "-analyzeduration".to_string(),
            "500000".into(),
            "-probesize".into(),
            "500000".into(),
            "-t".into(),
            format!("{window}"),
            "-i".into(),
            source.to_string(),
            "-vf".into(),
            format!("blackdetect=d={:.1}:pix_th=0.10,freezedetect=n=0.003:d={FREEZE_MIN_SECONDS}", window * 0.4),
            "-an".into(),
            "-f".into(),
            "null".into(),
            "-".into(),
        ],
    );
    args
}

pub fn frame_count(report: &str) -> usize {
    FRAMES.captures_iter(report).last().and_then(|c| c[1].parse().ok()).unwrap_or(0)
}

fn read_duration(report: &str) -> Option<f64> {
    TIME.captures_iter(report).last().map(|c| {
        c[1].parse::<f64>().unwrap_or(0.0) * 3600.0 + c[2].parse::<f64>().unwrap_or(0.0) * 60.0 + c[3].parse::<f64>().unwrap_or(0.0)
    })
}

pub fn frame_gap(report: &str) -> f64 {
    let frames = frame_count(report);
    match read_duration(report) {
        Some(duration) if frames >= 2 => duration / (frames - 1) as f64,
        _ => 0.0,
    }
}

pub fn analyse(report: &str, window: f64, min_frames: usize, key_frames: bool) -> Option<Verdict> {
    if frame_count(report) < min_frames {
        return None;
    }
    let black_seconds: f64 = BLACK.captures_iter(report).filter_map(|c| c[1].parse::<f64>().ok()).sum();
    let black = black_seconds > 0.0 && black_seconds >= window * WINDOW_SHARE;
    if key_frames && frame_gap(report) >= FREEZE_MIN_SECONDS {
        return Some(Verdict { black, frozen: false });
    }
    let starts: Vec<f64> = FREEZE_START.captures_iter(report).filter_map(|c| c[1].parse().ok()).collect();
    let closed: Vec<f64> = FREEZE_DURATION.captures_iter(report).filter_map(|c| c[1].parse().ok()).collect();
    let mut frozen_seconds: f64 = closed.iter().sum();
    if starts.len() > closed.len() {
        let end = read_duration(report).unwrap_or(window);
        frozen_seconds += (end - starts.last().copied().unwrap_or(0.0)).max(0.0);
    }
    Some(Verdict { black, frozen: !black && frozen_seconds >= window * WINDOW_SHARE })
}

pub fn needs_full_decode(report: &str, verdict: Option<Verdict>) -> bool {
    let long_gop = verdict.is_none() && frame_count(report) >= 1;
    let unknown_freeze = verdict.is_some_and(|v| !v.black) && frame_gap(report) >= FREEZE_MIN_SECONDS;
    long_gop || unknown_freeze
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Streaks {
    black: u32,
    frozen: u32,
}

impl Streaks {
    pub fn reset(&mut self) {
        *self = Streaks::default();
    }

    pub fn record(&mut self, verdict: Verdict) -> (bool, bool) {
        self.black = if verdict.black { self.black + 1 } else { 0 };
        self.frozen = if verdict.frozen { self.frozen + 1 } else { 0 };
        (self.black >= BLACK_CONFIRM, self.frozen >= FROZEN_CONFIRM)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEALTHY: &str = "frame=    9 fps=0.0 q=-0.0 Lsize=N/A time=00:00:05.00 bitrate=N/A speed= 754x\n";
    const BLACK_REPORT: &str = "[freezedetect @ 0x0] lavfi.freezedetect.freeze_start: 0\n[blackdetect @ 0x0] black_start:0 black_end:4 black_duration:4\nframe=    3 fps=0.0 q=-0.0 Lsize=N/A time=00:00:04.04 bitrate=N/A speed= 918x\n";
    const LIVE_GOP_2_5: &str = "[freezedetect @ 0x0] lavfi.freezedetect.freeze_start: 0\n[freezedetect @ 0x0] lavfi.freezedetect.freeze_duration: 2.48\n[freezedetect @ 0x0] lavfi.freezedetect.freeze_end: 2.48\n[freezedetect @ 0x0] lavfi.freezedetect.freeze_start: 2.48\n[freezedetect @ 0x0] lavfi.freezedetect.freeze_duration: 2.48\n[freezedetect @ 0x0] lavfi.freezedetect.freeze_end: 4.96\nframe=    3 fps=0.0 q=-0.0 Lsize=N/A time=00:00:05.00 bitrate=N/A speed= 767x\n";
    const FROZEN_GOP_2: &str = "[freezedetect @ 0x0] lavfi.freezedetect.freeze_start: 0\nframe=    3 fps=0.0 q=-0.0 Lsize=N/A time=00:00:04.04 bitrate=N/A speed= 930x\n";

    fn keys(report: &str) -> Option<Verdict> {
        analyse(report, SAMPLE_SECONDS, MIN_KEY_FRAMES, true)
    }

    fn verdict(black: bool, frozen: bool) -> Option<Verdict> {
        Some(Verdict { black, frozen })
    }

    #[test]
    fn the_probe_decodes_only_key_frames_and_looks_for_black_and_freeze() {
        let args = probe_args("http://127.0.0.1/index.m3u8", false);

        assert_eq!(args[args.iter().position(|a| a == "-skip_frame").unwrap() + 1], "nokey");
        let filter = &args[args.iter().position(|a| a == "-vf").unwrap() + 1];
        assert!(filter.contains("blackdetect") && filter.contains("freezedetect"));
        assert_eq!(args[args.iter().position(|a| a == "-t").unwrap() + 1], "6");
        assert!(!probe_args("x", true).contains(&"-skip_frame".to_string()));
    }

    #[test]
    fn the_report_tells_healthy_black_frozen_and_undetermined() {
        assert_eq!(keys(HEALTHY), verdict(false, false));
        assert_eq!(keys(BLACK_REPORT), verdict(true, false));
        assert_eq!(keys("Error opening input"), None);
        let still = "[freezedetect @ 0x0] lavfi.freezedetect.freeze_start: 0\nframe=   13 fps=0.0 q=-0.0 Lsize=N/A time=00:00:05.80 bitrate=N/A speed= 676x\n";
        assert_eq!(keys(still), verdict(false, true));
        let short = "[freezedetect @ 0x0] lavfi.freezedetect.freeze_start: 4.8\nframe=   13 fps=0.0 q=-0.0 Lsize=N/A time=00:00:05.80 bitrate=N/A speed= 676x\n";
        assert_eq!(keys(short), verdict(false, false));
        let closed = "[freezedetect @ 0x0] lavfi.freezedetect.freeze_start: 1\n[freezedetect @ 0x0] lavfi.freezedetect.freeze_duration: 4.5\n[freezedetect @ 0x0] lavfi.freezedetect.freeze_end: 5.5\nframe=   13 fps=0.0 q=-0.0 Lsize=N/A time=00:00:05.80 bitrate=N/A speed= 676x\n";
        assert_eq!(keys(closed), verdict(false, true));
    }

    #[test]
    fn a_live_channel_with_a_long_gop_is_not_frozen_by_the_key_frame_pass() {
        assert_eq!(keys(LIVE_GOP_2_5), verdict(false, false));
        assert_eq!(keys(FROZEN_GOP_2), verdict(false, false));
        assert_eq!(analyse(FROZEN_GOP_2, SAMPLE_SECONDS, MIN_KEY_FRAMES, false), verdict(false, true));
    }

    #[test]
    fn a_long_gop_or_unknown_freeze_asks_for_the_full_decode() {
        assert!(needs_full_decode(FROZEN_GOP_2, keys(FROZEN_GOP_2)));
        assert!(needs_full_decode("frame=    1 fps=0.0 time=00:00:00.04\n", None));
        assert!(!needs_full_decode(HEALTHY, keys(HEALTHY)));
        assert!(!needs_full_decode(BLACK_REPORT, keys(BLACK_REPORT)));
        assert!(!needs_full_decode("Error opening input", None));
    }

    #[test]
    fn two_readings_in_a_row_confirm_and_the_first_clean_one_lifts() {
        let mut streaks = Streaks::default();

        assert_eq!(streaks.record(Verdict { black: true, frozen: false }), (false, false));
        assert_eq!(streaks.record(Verdict { black: true, frozen: false }), (true, false));
        assert_eq!(streaks.record(Verdict { black: false, frozen: true }), (false, false));
        assert_eq!(streaks.record(Verdict { black: false, frozen: true }), (false, true));
        assert_eq!(streaks.record(Verdict { black: false, frozen: false }), (false, false));
    }
}
