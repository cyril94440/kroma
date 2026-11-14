use super::super::outcome::REASON_CHARS;
use super::*;

fn reason_of(outcome: Outcome) -> String {
    match outcome {
        Outcome::Unreadable(reason) => reason,
        Outcome::Read(..) => panic!("a refusal must not read as a description"),
        Outcome::NotProbed => panic!("a refusal must not read as a missing probe"),
        Outcome::Killed | Outcome::Network => {
            panic!("a refusal must not read as a killed probe")
        }
    }
}

#[test]
fn a_file_ffprobe_refuses_is_unreadable_and_keeps_ffprobes_own_reason() {
    let path = MediaInput::Local("/media/Dune (2021)/dune.mkv".into());
    let stderr = b"/media/Dune (2021)/dune.mkv: Invalid data found when processing input\n";

    let reason = reason_of(outcome_of(&path, Some(1), b"", stderr));

    assert_eq!(reason, "Invalid data found when processing input");
}

#[test]
fn the_reason_never_carries_the_path_the_client_is_not_shown() {
    let path = MediaInput::Local("/srv/media/private/x.mkv".into());
    let stderr = b"/srv/media/private/x.mkv: moov atom not found\n";

    let reason = reason_of(outcome_of(&path, Some(1), b"", stderr));

    assert!(!reason.contains("/srv/media"), "{reason}");
}

#[test]
fn a_refusal_with_nothing_to_say_still_names_the_status_it_failed_with() {
    let path = MediaInput::Local("/media/x.mkv".into());

    assert_eq!(
        reason_of(outcome_of(&path, Some(183), b"", b"  \n")),
        "ffprobe exited with status 183"
    );
}

#[test]
fn a_probe_a_signal_took_is_no_verdict_about_the_file() {
    let outcome = outcome_of(&MediaInput::Local("/media/x.mkv".into()), None, b"", b"");

    assert!(matches!(outcome, Outcome::Killed));
}

#[test]
fn a_reason_longer_than_the_cap_is_cut_to_it() {
    let path = MediaInput::Local("/media/x.mkv".into());
    let stderr = "e".repeat(REASON_CHARS * 2).into_bytes();

    let reason = reason_of(outcome_of(&path, Some(1), b"", &stderr));

    assert_eq!(reason.chars().count(), REASON_CHARS);
}

#[test]
fn output_kroma_cannot_parse_is_not_the_files_fault() {
    let outcome = outcome_of(
        &MediaInput::Local("/media/x.mkv".into()),
        Some(0),
        b"not json",
        b"",
    );

    assert!(matches!(outcome, Outcome::NotProbed));
}

#[test]
fn a_described_file_comes_back_with_its_streams_and_no_fault() {
    let stdout = br#"{"streams":[{"codec_type":"video","codec_name":"hevc","width":3840,"height":2160}]}"#;

    let outcome = outcome_of(
        &MediaInput::Local("/media/x.mkv".into()),
        Some(0),
        stdout,
        b"",
    );

    let Outcome::Read(result, _) = outcome else {
        panic!("a parsable description must read as one");
    };
    assert_eq!(
        result.video.as_ref().map(|v| v.codec.as_str()),
        Some("hevc")
    );
    assert!(result.unreadable.is_none());
}

#[test]
fn without_ffprobe_the_container_extension_still_names_a_codec() {
    let guessed = probe_file(Path::new("/media/x.webm"), false).expect("a guess");

    assert_eq!(guessed.video.map(|v| v.codec), Some("vp9".to_string()));
    assert!(guessed.unreadable.is_none());
}

#[test]
fn a_container_nothing_recognises_guesses_nothing() {
    let guessed = probe_file(Path::new("/media/x.bin"), false).expect("a guess");

    assert_eq!(guessed.video.map(|v| v.codec), Some("unknown".to_string()));
}
