use std::sync::LazyLock;

use regex::Regex;

pub const CAPTURE_SECONDS: f64 = 6.0;
pub const BLACK_SHARE: f64 = 0.6;

static BLACK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"black_duration:([\d.]+)").unwrap());

pub fn capture_args(url: &str, user_agent: &str, folder: &str) -> Vec<String> {
    let seconds = format!("{CAPTURE_SECONDS}");
    let mut args: Vec<String> = ["-v", "info", "-nostdin", "-user_agent", user_agent, "-fflags", "nobuffer", "-skip_frame", "nokey", "-analyzeduration", "500000", "-probesize", "500000", "-t"].map(String::from).to_vec();
    args.extend([seconds.clone(), "-i".into(), url.into(), "-vf".into(), "blackdetect=d=0.3:pix_th=0.10,fps=1,scale=480:-1".into(), "-frames:v".into(), format!("{CAPTURE_SECONDS:.0}"), "-q:v".into(), "6".into(), "-y".into(), format!("{folder}/f_%02d.jpg")]);
    args
}

pub fn black_in_report(report: &str, seconds: f64) -> bool {
    let total: f64 = BLACK.captures_iter(report).filter_map(|c| c[1].parse::<f64>().ok()).sum();
    total > 0.0 && total >= seconds * BLACK_SHARE
}

pub fn pick_frames(blobs: &[Vec<u8>]) -> Vec<usize> {
    let Some(first) = blobs.first() else { return Vec::new() };
    match blobs.iter().rposition(|blob| blob != first) {
        Some(last) => vec![0, last],
        None => vec![0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_capture_decodes_key_frames_only_over_six_seconds_and_keeps_one_frame_a_second() {
        let args = capture_args("http://x/1.ts", "UA", "/w");

        assert_eq!(args[args.iter().position(|a| a == "-skip_frame").unwrap() + 1], "nokey");
        assert_eq!(args[args.iter().position(|a| a == "-t").unwrap() + 1], "6");
        assert_eq!(args[args.iter().position(|a| a == "-frames:v").unwrap() + 1], "6");
        assert!(args[args.iter().position(|a| a == "-vf").unwrap() + 1].contains("blackdetect=d=0.3:pix_th=0.10,fps=1,scale=480:-1"));
        assert_eq!(args.last().map(String::as_str), Some("/w/f_%02d.jpg"));
    }

    #[test]
    fn a_picture_is_black_when_black_covers_sixty_percent_of_the_sample() {
        let report = |seconds: &str| format!("[blackdetect @ 0x0] black_start:0 black_end:4 black_duration:{seconds}\n");

        assert!(black_in_report(&report("3.6"), CAPTURE_SECONDS));
        assert!(!black_in_report(&report("3.5"), CAPTURE_SECONDS));
        assert!(!black_in_report("frame=3", CAPTURE_SECONDS));
    }

    #[test]
    fn the_first_and_the_last_distinct_frames_are_kept_and_a_frozen_one_is_alone() {
        let frame = |byte: u8| vec![byte; 4];

        assert_eq!(pick_frames(&[frame(1), frame(2), frame(1), frame(3), frame(3)]), [0, 4]);
        assert_eq!(pick_frames(&[frame(1), frame(1), frame(1)]), [0]);
        assert_eq!(pick_frames(&[frame(1), frame(2), frame(1)]), [0, 1]);
        assert!(pick_frames(&[]).is_empty());
    }
}
