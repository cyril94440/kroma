use std::path::{Path, PathBuf};

use crate::clipbuf::*;

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
