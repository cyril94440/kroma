use kroma_module_wire::ColorInfo;

use super::*;

const HDR10_PLUS_FRAME: &str = r#"{"frames":[{"side_data_list":[
    {"side_data_type":"Mastering display metadata"},
    {"side_data_type":"Content light level metadata","max_content":1000,"max_average":200},
    {"side_data_type":"HDR Dynamic Metadata SMPTE2094-40 (HDR10+)","application version":1}]}]}"#;

const DOLBY_FRAME: &str = r#"{"frames":[{"side_data_list":[
    {"side_data_type":"Mastering display metadata"},
    {"side_data_type":"Dolby Vision RPU Data"},
    {"side_data_type":"Dolby Vision Metadata"}]}]}"#;

fn frames(json: &str) -> FfFrames {
    serde_json::from_str(json).unwrap()
}

fn video(format: Option<HdrFormat>, transfer: &str) -> VideoStream {
    VideoStream {
        codec: "hevc".into(),
        width: Some(3840),
        height: Some(2160),
        hdr: format.is_some(),
        bit_depth: Some(10),
        hdr_format: format,
        dolby_vision_profile: None,
        dolby_vision_level: None,
        dolby_vision_compatibility: None,
        dolby_vision_enhancement_layer: false,
        color: Some(ColorInfo {
            transfer: Some(transfer.into()),
            ..ColorInfo::default()
        }),
    }
}

#[test]
fn the_smpte_2094_40_side_data_ffprobe_prints_marks_a_frame_as_hdr10_plus() {
    assert!(carries(&frames(HDR10_PLUS_FRAME)));
}

#[test]
fn a_frame_with_only_static_or_dolby_metadata_is_not_hdr10_plus() {
    assert!(!carries(&frames(DOLBY_FRAME)));
    assert!(!carries(&frames(r#"{"frames":[]}"#)));
    assert!(!carries(&frames(r#"{"frames":[{}]}"#)));
}

#[test]
fn a_newer_ffprobe_spelling_of_the_same_side_data_still_counts() {
    assert!(carries(&frames(
        r#"{"frames":[{"side_data_list":[{"side_data_type":"HDR10+ Dynamic Metadata"}]}]}"#
    )));
}

#[test]
fn hdr10_becomes_hdr10_plus_only_when_the_first_frame_says_so() {
    let mut plus = video(Some(HdrFormat::Hdr10), "smpte2084");
    let mut plain = video(Some(HdrFormat::Hdr10), "smpte2084");

    upgrade(&mut plus, || true);
    upgrade(&mut plain, || false);

    assert_eq!(plus.hdr_format, Some(HdrFormat::Hdr10Plus));
    assert_eq!(plain.hdr_format, Some(HdrFormat::Hdr10));
}

#[test]
fn only_a_pq_hdr10_stream_is_ever_read_a_second_time() {
    for stream in [
        video(None, "bt709"),
        video(Some(HdrFormat::Hlg), "arib-std-b67"),
        video(Some(HdrFormat::DolbyVision), "smpte2084"),
        video(Some(HdrFormat::Hdr10), "unknown"),
    ] {
        let mut stream = stream;
        let before = stream.hdr_format;

        upgrade(&mut stream, || panic!("{before:?} must not cost a second pass"));

        assert_eq!(stream.hdr_format, before);
    }
}

#[test]
fn a_file_ffprobe_cannot_open_reads_as_no_hdr10_plus() {
    assert!(!first_frame_carries(Path::new("/nonexistent/clip.mkv"), 0));
}

#[test]
fn a_probe_that_outlives_its_budget_is_killed_and_reads_as_nothing() {
    let started = Instant::now();

    let out = run(
        "sh",
        &["-c".into(), "sleep 30".into()],
        Duration::from_millis(100),
    );

    assert!(out.is_none());
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn a_probe_that_finishes_in_time_hands_back_what_it_printed() {
    let out = run(
        "sh",
        &["-c".into(), "printf '{}'".into()],
        Duration::from_secs(5),
    );

    assert_eq!(out.as_deref(), Some(b"{}".as_slice()));
}

#[test]
fn the_pass_reads_one_frame_of_the_stream_it_was_asked_about() {
    let rendered: Vec<String> = args(Path::new("/media/x.mkv"), 2)
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let joined = rendered.join(" ");

    assert!(joined.contains("-select_streams 2"), "{joined}");
    assert!(joined.contains("-read_intervals %+#1"), "{joined}");
    assert!(joined.contains("-protocol_whitelist file"), "{joined}");
}
