use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use crate::epg_parse::days_from_civil;

pub const BUFFER_SECONDS: i64 = 600;
pub const BUFFER_MAX_BYTES: u64 = 1024 * 1024 * 1024;
pub const CLIP_MAX_SECONDS: i64 = 240;
pub const CLIPS_MAX_BYTES: u64 = 6 * 1024 * 1024 * 1024;
pub const CLIPS_MAX_DAYS: i64 = 45;
pub const CLIPS_DIR: &str = "clips";
pub const BUFFER_DIR: &str = "buffer";
pub const INIT_FILE: &str = "init.mp4";

#[derive(Debug, Clone, PartialEq)]
pub struct PlaylistSegment {
    pub name: String,
    pub duration_ms: i64,
    pub start_ms: Option<i64>,
}

pub fn parse_program_date_time(text: &str) -> Option<i64> {
    let text = text.trim();
    let (date, rest) = text.split_once('T')?;
    let mut parts = date.split('-');
    let (year, month, day): (i64, i64, i64) = (parts.next()?.parse().ok()?, parts.next()?.parse().ok()?, parts.next()?.parse().ok()?);
    let split = rest.find(['Z', '+', '-']).unwrap_or(rest.len());
    let (clock, zone) = rest.split_at(split);
    let mut fields = clock.split(':');
    let hour: i64 = fields.next()?.parse().ok()?;
    let minute: i64 = fields.next()?.parse().ok()?;
    let seconds: f64 = fields.next()?.parse().ok()?;
    let offset_seconds = match zone.as_bytes().first() {
        Some(b'+') | Some(b'-') => {
            let digits: String = zone[1..].chars().filter(char::is_ascii_digit).collect();
            let (hours, minutes): (i64, i64) = (digits.get(..2)?.parse().ok()?, digits.get(2..4).unwrap_or("00").parse().ok()?);
            (hours * 3600 + minutes * 60) * if zone.starts_with('-') { -1 } else { 1 }
        }
        _ => 0,
    };
    let whole = days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 - offset_seconds;
    Some(whole * 1000 + (seconds * 1000.0).round() as i64)
}

pub fn parse_playlist(text: &str) -> (u64, Vec<PlaylistSegment>) {
    let mut first_sequence = 0;
    let mut duration = None;
    let mut start = None;
    let mut out: Vec<PlaylistSegment> = Vec::new();
    for line in text.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("#EXT-X-MEDIA-SEQUENCE:") {
            first_sequence = rest.trim().parse().unwrap_or(0);
        } else if let Some(rest) = line.strip_prefix("#EXTINF:") {
            duration = rest.split(',').next().and_then(|d| d.trim().parse::<f64>().ok()).map(|d| (d * 1000.0).round() as i64);
        } else if let Some(rest) = line.strip_prefix("#EXT-X-PROGRAM-DATE-TIME:") {
            start = parse_program_date_time(rest);
        } else if !line.is_empty() && !line.starts_with('#') {
            if let Some(duration_ms) = duration.take() {
                let after_previous = out.last().and_then(|previous| previous.start_ms.map(|s| s + previous.duration_ms));
                out.push(PlaylistSegment { name: line.to_string(), duration_ms, start_ms: start.take().or(after_previous) });
            }
        }
    }
    (first_sequence, out)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub name: String,
    pub seq: u64,
    pub start_ms: i64,
    pub duration_ms: i64,
    pub bytes: u64,
}

impl Segment {
    pub fn end_ms(&self) -> i64 {
        self.start_ms + self.duration_ms
    }
}

#[derive(Debug, Clone)]
pub struct BufferRun {
    pub folder: String,
    pub dir: PathBuf,
    pub segments: VecDeque<Segment>,
}

#[derive(Debug, Default)]
pub struct Buffer {
    runs: Vec<BufferRun>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Selection {
    pub dir: PathBuf,
    pub segments: Vec<Segment>,
}

impl Selection {
    pub fn from_ms(&self) -> i64 {
        self.segments.first().map_or(0, |s| s.start_ms)
    }

    pub fn duration_ms(&self) -> i64 {
        self.segments.iter().map(|s| s.duration_ms).sum()
    }

    pub fn playlist(&self) -> String {
        let target = self.segments.iter().map(|s| (s.duration_ms + 999) / 1000).max().unwrap_or(1);
        let mut text = format!("#EXTM3U\n#EXT-X-VERSION:7\n#EXT-X-TARGETDURATION:{target}\n#EXT-X-MEDIA-SEQUENCE:0\n#EXT-X-MAP:URI=\"{}\"\n", self.dir.join(INIT_FILE).to_string_lossy());
        for segment in &self.segments {
            text.push_str(&format!("#EXTINF:{:.3},\n{}\n", segment.duration_ms as f64 / 1000.0, self.dir.join(&segment.name).to_string_lossy()));
        }
        text.push_str("#EXT-X-ENDLIST\n");
        text
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct Purged {
    pub files: Vec<PathBuf>,
    pub folders: Vec<PathBuf>,
}

fn touches(segment: &Segment, from_ms: i64, to_ms: i64) -> bool {
    segment.end_ms() > from_ms && segment.start_ms < to_ms
}

fn chains(segments: Vec<Segment>) -> Vec<Vec<Segment>> {
    let mut out: Vec<Vec<Segment>> = Vec::new();
    for segment in segments {
        match out.last_mut() {
            Some(chain) if chain.last().is_some_and(|last| last.seq + 1 == segment.seq) => chain.push(segment),
            _ => out.push(vec![segment]),
        }
    }
    out
}

impl Buffer {
    pub fn runs(&self) -> &[BufferRun] {
        &self.runs
    }

    pub fn observe(&mut self, folder: &str, dir: &Path, playlist: &str, size_of: impl Fn(&str) -> Option<u64>) {
        let (first_sequence, listed) = parse_playlist(playlist);
        let index = match self.runs.iter().position(|run| run.folder == folder) {
            Some(index) => index,
            None => {
                self.runs.push(BufferRun { folder: folder.to_string(), dir: dir.to_path_buf(), segments: VecDeque::new() });
                self.runs.len() - 1
            }
        };
        let run = &mut self.runs[index];
        for (offset, entry) in listed.iter().enumerate() {
            let seq = first_sequence + offset as u64;
            let known = run.segments.back().is_some_and(|last| last.seq >= seq);
            let (Some(start_ms), false) = (entry.start_ms, known) else { continue };
            let Some(bytes) = size_of(&entry.name) else { continue };
            run.segments.push_back(Segment { name: entry.name.clone(), seq, start_ms, duration_ms: entry.duration_ms, bytes });
        }
    }

    pub fn move_run(&mut self, folder: &str, dir: &Path) {
        if let Some(run) = self.runs.iter_mut().find(|run| run.folder == folder) {
            run.dir = dir.to_path_buf();
        }
    }

    pub fn has_segments(&self, folder: &str) -> bool {
        self.runs.iter().any(|run| run.folder == folder && !run.segments.is_empty())
    }

    pub fn drop_run(&mut self, folder: &str) -> Option<PathBuf> {
        let at = self.runs.iter().position(|run| run.folder == folder)?;
        Some(self.runs.remove(at).dir)
    }

    pub fn bounds(&self) -> Option<(i64, i64)> {
        let first = self.runs.iter().filter_map(|run| run.segments.front()).map(|s| s.start_ms).min()?;
        let last = self.runs.iter().filter_map(|run| run.segments.back()).map(|s| s.end_ms()).max()?;
        Some((first, last))
    }

    pub fn bytes(&self) -> u64 {
        self.runs.iter().flat_map(|run| run.segments.iter()).map(|s| s.bytes).sum()
    }

    pub fn choose(&self, from_ms: i64, to_ms: i64, pivot_ms: Option<i64>) -> Option<Selection> {
        let pivot = pivot_ms.unwrap_or((from_ms + to_ms) / 2);
        let mut best: Option<(bool, usize, &BufferRun, Vec<Segment>)> = None;
        for run in &self.runs {
            let touching: Vec<Segment> = run.segments.iter().filter(|s| touches(s, from_ms, to_ms)).cloned().collect();
            if touching.is_empty() {
                continue;
            }
            let has_pivot = touching.iter().any(|s| s.start_ms <= pivot && pivot < s.end_ms());
            let rank = (has_pivot, touching.len());
            if best.as_ref().is_none_or(|(held, count, _, _)| rank > (*held, *count)) {
                best = Some((rank.0, rank.1, run, touching));
            }
        }
        let (_, _, run, touching) = best?;
        let mut parts = chains(touching);
        let at = parts.iter().position(|chain| chain.iter().any(|s| s.start_ms <= pivot && pivot < s.end_ms())).or_else(|| (0..parts.len()).max_by_key(|i| parts[*i].len()))?;
        Some(Selection { dir: run.dir.clone(), segments: parts.swap_remove(at) })
    }

    pub fn purge(&mut self, now_ms: i64, current: Option<&str>) -> Purged {
        let mut purged = Purged::default();
        let oldest_kept = now_ms - BUFFER_SECONDS * 1000;
        for run in &mut self.runs {
            while run.segments.front().is_some_and(|s| s.end_ms() < oldest_kept) {
                if let Some(segment) = run.segments.pop_front() {
                    purged.files.push(run.dir.join(segment.name));
                }
            }
        }
        let mut total = self.bytes();
        while total > BUFFER_MAX_BYTES {
            let Some(run) = self.runs.iter_mut().filter(|run| !run.segments.is_empty()).min_by_key(|run| run.segments.front().map_or(i64::MAX, |s| s.start_ms)) else { break };
            let Some(segment) = run.segments.pop_front() else { break };
            total -= segment.bytes.min(total);
            purged.files.push(run.dir.join(segment.name));
        }
        self.runs.retain(|run| {
            let empty = run.segments.is_empty();
            if empty && current != Some(run.folder.as_str()) {
                purged.folders.push(run.dir.clone());
            }
            !empty || current == Some(run.folder.as_str())
        });
        purged
    }
}

pub fn cut_args(playlist: &str, output: &str) -> Vec<String> {
    ["-hide_banner", "-loglevel", "error", "-nostdin", "-y", "-allowed_extensions", "ALL", "-i", playlist, "-c", "copy", "-movflags", "+faststart", "-f", "mp4", output].map(String::from).to_vec()
}

pub fn valid_window(from_ms: i64, to_ms: i64) -> bool {
    to_ms > from_ms && to_ms - from_ms <= CLIP_MAX_SECONDS * 1000
}

pub fn valid_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    (3..=81).contains(&bytes.len())
        && bytes[0].is_ascii_alphanumeric()
        && bytes.iter().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'_' || *b == b'-')
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipFile {
    pub path: PathBuf,
    pub modified_s: i64,
    pub bytes: u64,
}

pub fn clips_to_purge(mut clips: Vec<ClipFile>, now_s: i64, max_bytes: u64, max_days: i64) -> Vec<PathBuf> {
    clips.sort_by_key(|clip| clip.modified_s);
    let mut total: u64 = clips.iter().map(|clip| clip.bytes).sum();
    let mut out = Vec::new();
    for clip in clips {
        if now_s - clip.modified_s > max_days * 86_400 || total > max_bytes {
            total -= clip.bytes.min(total);
            out.push(clip.path);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIR: &str = "/live/buffer/run1";
    const T0: i64 = 1_794_696_000_000;

    fn iso(ms: i64) -> String {
        let days = ms.div_euclid(86_400_000);
        let rest = ms.rem_euclid(86_400_000);
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = yoe + era * 400 + i64::from(month <= 2);
        format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}+0000", rest / 3_600_000, rest / 60_000 % 60, rest / 1000 % 60, rest % 1000)
    }

    fn playlist(first_sequence: u64, start_ms: i64, count: usize) -> String {
        let mut text = format!("#EXTM3U\n#EXT-X-VERSION:7\n#EXT-X-MEDIA-SEQUENCE:{first_sequence}\n#EXT-X-MAP:URI=\"init.mp4\"\n");
        for k in 0..count {
            text.push_str(&format!("#EXTINF:1.000000,\n#EXT-X-PROGRAM-DATE-TIME:{}\nseg{:05}.m4s\n", iso(start_ms + k as i64 * 1000), first_sequence + k as u64));
        }
        text
    }

    fn filled(first_sequence: u64, start_ms: i64, count: usize) -> Buffer {
        let mut buffer = Buffer::default();
        buffer.observe("run1", Path::new(DIR), &playlist(first_sequence, start_ms, count), |_| Some(1000));
        buffer
    }

    #[test]
    fn a_program_date_time_gives_epoch_milliseconds_in_any_zone_spelling() {
        let utc = parse_program_date_time("2026-11-14T22:51:13.101+0000");

        assert_eq!(utc, Some(1_794_696_673_101));
        assert_eq!(parse_program_date_time("2026-11-14T22:51:13.101Z"), utc);
        assert_eq!(parse_program_date_time("2026-11-14T23:51:13.101+01:00"), utc);
        assert_eq!(parse_program_date_time("2026-11-14T20:51:13.101-0200"), utc);
        assert_eq!(parse_program_date_time("nonsense"), None);
        assert_eq!(parse_program_date_time(&iso(T0 + 12_345)), Some(T0 + 12_345));
    }

    #[test]
    fn a_playlist_gives_names_durations_and_start_times_even_when_one_stamp_is_missing() {
        let text = "#EXTM3U\n#EXT-X-MEDIA-SEQUENCE:7\n#EXTINF:2.0,\n#EXT-X-PROGRAM-DATE-TIME:2026-11-14T22:51:13.000+0000\na.m4s\n#EXTINF:1.5,\nb.m4s\n";

        let (first, segments) = parse_playlist(text);

        assert_eq!(first, 7);
        let first_start = segments[0].start_ms.unwrap();
        assert_eq!(segments.iter().map(|s| (s.name.as_str(), s.duration_ms, s.start_ms.unwrap() - first_start)).collect::<Vec<_>>(), [("a.m4s", 2000, 0), ("b.m4s", 1500, 2000)]);
    }

    #[test]
    fn observing_an_overlapping_playlist_adds_only_the_new_segments() {
        let mut buffer = filled(0, T0, 5);

        buffer.observe("run1", Path::new(DIR), &playlist(2, T0 + 2000, 5), |_| Some(1000));

        let seqs: Vec<u64> = buffer.runs()[0].segments.iter().map(|s| s.seq).collect();
        assert_eq!(seqs, [0, 1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn a_window_inside_one_run_selects_the_whole_segments_that_touch_it() {
        let buffer = filled(0, T0, 30);

        let selection = buffer.choose(T0 + 10_500, T0 + 15_200, None).unwrap();

        assert_eq!(selection.segments.first().map(|s| s.seq), Some(10));
        assert_eq!(selection.segments.last().map(|s| s.seq), Some(15));
        assert_eq!(selection.duration_ms(), 6000);
        assert!(buffer.choose(T0 + 500_000, T0 + 520_000, None).is_none());
    }

    #[test]
    fn a_clip_never_straddles_two_runs_and_follows_the_run_of_the_pivot() {
        let mut buffer = filled(0, T0, 10);
        buffer.observe("run2", Path::new("/live/public/run2"), &playlist(0, T0 + 14_000, 10), |_| Some(1000));

        let first = buffer.choose(T0 + 3000, T0 + 25_000, Some(T0 + 5000)).unwrap();
        let second = buffer.choose(T0 + 3000, T0 + 25_000, Some(T0 + 20_000)).unwrap();

        assert_eq!(first.dir, PathBuf::from(DIR));
        assert_eq!(second.dir, PathBuf::from("/live/public/run2"));
    }

    #[test]
    fn a_hole_in_the_sequence_splits_the_selection_and_keeps_the_chain_around_the_pivot() {
        let mut buffer = Buffer::default();
        buffer.observe("run1", Path::new(DIR), &playlist(0, T0, 5), |_| Some(10));
        buffer.observe("run1", Path::new(DIR), &playlist(8, T0 + 8000, 5), |_| Some(10));

        let selection = buffer.choose(T0, T0 + 13_000, Some(T0 + 10_500)).unwrap();

        assert_eq!(selection.segments.iter().map(|s| s.seq).collect::<Vec<_>>(), [8, 9, 10, 11, 12]);
    }

    #[test]
    fn the_buffer_forgets_segments_older_than_ten_minutes_and_empty_old_runs() {
        let mut buffer = filled(0, T0, 5);
        buffer.observe("run2", Path::new("/live/public/run2"), &playlist(0, T0 + 700_000, 5), |_| Some(10));

        let purged = buffer.purge(T0 + BUFFER_SECONDS * 1000 + 10_000, Some("run2"));

        assert_eq!(purged.files.len(), 5);
        assert_eq!(purged.folders, [PathBuf::from(DIR)]);
        assert_eq!(buffer.runs().len(), 1);
        assert_eq!(buffer.bounds(), Some((T0 + 700_000, T0 + 705_000)));
    }

    #[test]
    fn the_buffer_stays_inside_its_disk_budget_by_dropping_the_oldest_first() {
        let mut buffer = Buffer::default();
        buffer.observe("run1", Path::new(DIR), &playlist(0, T0, 4), |_| Some(BUFFER_MAX_BYTES / 2));

        let purged = buffer.purge(T0 + 10_000, Some("run1"));

        assert_eq!(purged.files.len(), 2);
        assert!(buffer.bytes() <= BUFFER_MAX_BYTES);
        assert_eq!(buffer.runs()[0].segments.front().map(|s| s.seq), Some(2));
    }

    #[test]
    fn the_cut_playlist_lists_the_init_and_every_segment_and_ends() {
        let buffer = filled(0, T0, 3);
        let selection = buffer.choose(T0, T0 + 3000, None).unwrap();

        let text = selection.playlist();

        assert!(text.contains(&format!("#EXT-X-MAP:URI=\"{DIR}/init.mp4\"")));
        assert_eq!(text.matches("#EXTINF").count(), 3);
        assert!(text.ends_with("#EXT-X-ENDLIST\n"));
    }

    #[test]
    fn the_cut_is_a_stream_copy_into_mp4() {
        let args = cut_args("/p/cut.m3u8", "/p/out.mp4");

        assert_eq!(args[args.iter().position(|a| a == "-c").unwrap() + 1], "copy");
        assert!(args.contains(&"+faststart".to_string()));
        assert_eq!(args.last().map(String::as_str), Some("/p/out.mp4"));
    }

    #[test]
    fn only_a_window_of_up_to_four_minutes_and_a_plain_name_are_accepted() {
        assert!(valid_window(0, 240_000) && !valid_window(0, 240_001) && !valid_window(5, 5) && !valid_window(9, 3));
        assert!(valid_name("espn-1-but-2") && valid_name("a_b"));
        for bad in ["../etc", "ab", "Upper", "-lead", "with space", ""] {
            assert!(!valid_name(bad), "{bad}");
        }
    }

    #[test]
    fn clips_are_purged_when_too_old_then_oldest_first_until_under_the_cap() {
        let clip = |name: &str, age_days: i64| ClipFile { path: PathBuf::from(name), modified_s: 1_000_000 - age_days * 86_400, bytes: 1000 };
        let clips = vec![clip("c0", 60), clip("c1", 2), clip("c2", 1)];

        assert_eq!(clips_to_purge(clips.clone(), 1_000_000, CLIPS_MAX_BYTES, CLIPS_MAX_DAYS), [PathBuf::from("c0")]);
        assert_eq!(clips_to_purge(clips, 1_000_000, 1500, CLIPS_MAX_DAYS), [PathBuf::from("c0"), PathBuf::from("c1")]);
    }
}
