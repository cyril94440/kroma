use std::path::Path;
use std::time::{Duration, SystemTime};

pub const STALE_AFTER: Duration = Duration::from_secs(6);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Window {
    pub segments: usize,
    pub kbps: f64,
}

#[derive(Debug, Default)]
pub struct ZapTiming {
    pub started: Option<std::time::Instant>,
    pub probe_done: Option<std::time::Instant>,
    pub stopped: Option<std::time::Instant>,
    pub launched: Option<std::time::Instant>,
}

pub fn segments_of(playlist: &str) -> Vec<(String, f64)> {
    let mut out = Vec::new();
    let mut duration = None;
    for line in playlist.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("#EXTINF:") {
            duration = rest.split(',').next().and_then(|d| d.parse::<f64>().ok());
        } else if !line.is_empty() && !line.starts_with('#') {
            if let Some(d) = duration.take() {
                out.push((line.to_string(), d));
            }
        }
    }
    out
}

pub fn read_window(folder: &Path) -> Option<Window> {
    let text = std::fs::read_to_string(folder.join(crate::cmd::PLAYLIST_FILE)).ok()?;
    let segments = segments_of(&text);
    let mut bytes = 0u64;
    let mut seconds = 0.0;
    let mut counted = 0;
    for (name, duration) in &segments {
        if let Ok(meta) = std::fs::metadata(folder.join(name)) {
            bytes += meta.len();
            seconds += duration;
            counted += 1;
        }
    }
    if counted == 0 || seconds <= 0.0 {
        return Some(Window { segments: 0, kbps: 0.0 });
    }
    Some(Window { segments: counted, kbps: bytes as f64 * 8.0 / seconds / 1000.0 })
}

pub fn newest_segment_age(folder: &Path) -> Option<Duration> {
    let text = std::fs::read_to_string(folder.join(crate::cmd::PLAYLIST_FILE)).ok()?;
    let (name, _) = segments_of(&text).pop()?;
    let modified = std::fs::metadata(folder.join(name)).ok()?.modified().ok()?;
    SystemTime::now().duration_since(modified).ok().or(Some(Duration::ZERO))
}

pub fn out_time(line: &str) -> Option<f64> {
    let value = line.strip_prefix("out_time=")?.trim();
    let mut parts = value.split(':');
    let hours: f64 = parts.next()?.parse().ok()?;
    let minutes: f64 = parts.next()?.parse().ok()?;
    let seconds: f64 = parts.next()?.parse().ok()?;
    Some(hours * 3600.0 + minutes * 60.0 + seconds)
}

pub fn lines(carry: &mut String, chunk: &[u8]) -> Vec<String> {
    carry.push_str(&String::from_utf8_lossy(chunk));
    let mut out = Vec::new();
    while let Some(at) = carry.find('\n') {
        let line: String = carry.drain(..=at).collect();
        out.push(line.trim_end().to_string());
    }
    if carry.len() > 65_536 {
        carry.clear();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_playlist_gives_its_segments_with_their_durations() {
        let playlist = "#EXTM3U\n#EXT-X-MAP:URI=\"init.mp4\"\n#EXTINF:2.000000,\n#EXT-X-PROGRAM-DATE-TIME:2026-11-14T22:51:13.101+0000\nseg00000.m4s\n#EXTINF:1.040000,\nseg00001.m4s\n";

        assert_eq!(segments_of(playlist), [("seg00000.m4s".to_string(), 2.0), ("seg00001.m4s".to_string(), 1.04)]);
    }

    #[test]
    fn the_source_rate_is_what_the_window_carries_per_second() {
        let dir = kroma_module_sdk::testing::temp_dir("live-media");
        std::fs::write(dir.path().join("index.m3u8"), "#EXTM3U\n#EXTINF:2.0,\na.m4s\n#EXTINF:2.0,\nb.m4s\n#EXTINF:2.0,\nmissing.m4s\n").unwrap();
        std::fs::write(dir.path().join("a.m4s"), vec![0u8; 125_000]).unwrap();
        std::fs::write(dir.path().join("b.m4s"), vec![0u8; 125_000]).unwrap();

        let window = read_window(dir.path()).unwrap();

        assert_eq!(window.segments, 2);
        assert!((window.kbps - 500.0).abs() < 0.01, "{}", window.kbps);
        assert!(newest_segment_age(dir.path()).is_none());
        assert_eq!(read_window(&dir.path().join("nope")), None);
    }

    #[test]
    fn progress_lines_are_read_in_whole_and_the_partial_one_waits() {
        let mut carry = String::new();

        let first = lines(&mut carry, b"frame=1\nout_time=00:01:23.456789\nprogr");
        let second = lines(&mut carry, b"ess=continue\n");

        assert_eq!(first, ["frame=1", "out_time=00:01:23.456789"]);
        assert_eq!(second, ["progress=continue"]);
        assert!((out_time("out_time=00:01:23.456789").unwrap() - 83.456789).abs() < 1e-9);
        assert_eq!(out_time("out_time=N/A"), None);
        assert_eq!(out_time("total_size=3"), None);
    }
}
