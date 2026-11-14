use super::*;

fn file(minutes: u64, width: u32, height: u32) -> Facts {
    Facts {
        unreadable: None,
        duration_ms: Some(minutes * 60_000),
        width: Some(width),
        height: Some(height),
    }
}

fn promise(runtime_min: Option<u32>, claimed_height: Option<u32>) -> Promise {
    Promise {
        runtime_min,
        claimed_height,
        ..Promise::default()
    }
}

fn is_fake(verdict: &Verdict) -> bool {
    matches!(verdict, Verdict::Fake(_))
}

#[test]
fn a_file_that_matches_its_title_and_its_name_is_genuine() {
    assert_eq!(
        judge(&file(155, 1920, 1080), &promise(Some(155), Some(1080))),
        Verdict::Genuine
    );
}

#[test]
fn an_unreadable_file_is_fake_whatever_else_is_known() {
    let facts = Facts {
        unreadable: Some("moov atom not found".into()),
        ..file(155, 1920, 1080)
    };

    assert!(is_fake(&judge(&facts, &promise(None, None))));
}

#[test]
fn a_file_with_no_video_is_fake() {
    let facts = Facts {
        duration_ms: Some(9_000_000),
        ..Facts::default()
    };

    assert!(is_fake(&judge(&facts, &promise(None, None))));
}

#[test]
fn a_sample_standing_in_for_a_feature_is_fake() {
    assert!(is_fake(&judge(
        &file(4, 1920, 1080),
        &promise(Some(155), None)
    )));
}

#[test]
fn a_longer_cut_is_still_the_title_and_a_double_length_file_is_not() {
    assert_eq!(
        judge(&file(190, 1920, 1080), &promise(Some(155), None)),
        Verdict::Genuine
    );
    assert!(is_fake(&judge(
        &file(400, 1920, 1080),
        &promise(Some(155), None)
    )));
}

#[test]
fn a_title_with_no_known_runtime_is_not_judged_on_duration() {
    assert_eq!(
        judge(&file(4, 1920, 1080), &promise(None, None)),
        Verdict::Genuine
    );
}

#[test]
fn a_file_missing_its_duration_is_not_judged_on_duration() {
    let facts = Facts {
        duration_ms: None,
        ..file(0, 1920, 1080)
    };

    assert_eq!(judge(&facts, &promise(Some(155), None)), Verdict::Genuine);
}

#[test]
fn one_tier_below_the_claim_passes_and_two_do_not() {
    assert_eq!(
        judge(&file(100, 1280, 720), &promise(Some(100), Some(1080))),
        Verdict::Genuine
    );
    assert!(is_fake(&judge(
        &file(100, 1280, 720),
        &promise(Some(100), Some(2160))
    )));
    assert!(is_fake(&judge(
        &file(100, 640, 480),
        &promise(Some(100), Some(1080))
    )));
}

#[test]
fn a_letterboxed_1080p_is_not_taken_for_720p() {
    assert_eq!(
        judge(&file(100, 1920, 800), &promise(Some(100), Some(1080))),
        Verdict::Genuine
    );
    assert_eq!(
        judge(&file(100, 3840, 1600), &promise(Some(100), Some(2160))),
        Verdict::Genuine
    );
}

#[test]
fn a_better_file_than_claimed_passes() {
    assert_eq!(
        judge(&file(100, 3840, 2160), &promise(Some(100), Some(720))),
        Verdict::Genuine
    );
}

#[test]
fn the_reason_names_what_was_found_and_what_was_promised() {
    let Verdict::Fake(reason) = judge(&file(4, 1920, 1080), &promise(Some(155), None)) else {
        panic!("not fake")
    };

    assert!(
        reason.contains("4 minutes") && reason.contains("155"),
        "{reason}"
    );
}

#[test]
fn a_double_episode_file_is_held_to_two_runtimes() {
    let two = Promise {
        episodes: 2,
        ..promise(Some(45), None)
    };

    assert_eq!(judge(&file(90, 1920, 1080), &two), Verdict::Genuine);
    assert!(is_fake(&judge(&file(90, 1920, 1080), &promise(Some(45), None))));
}

#[test]
fn a_part_of_a_multi_part_movie_is_not_held_to_the_runtime() {
    let part = Promise {
        multi_part: true,
        ..promise(Some(150), None)
    };

    assert_eq!(judge(&file(75, 1920, 1080), &part), Verdict::Genuine);
    assert!(is_fake(&judge(&file(75, 1920, 1080), &promise(Some(150), None))));
}
