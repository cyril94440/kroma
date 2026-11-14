use serde_json::{json, Value};

use crate::db::pipeline::UNREADABLE_SIG;
use crate::services::jobs::JobContext;
use crate::state::SharedState;
use crate::test_support::{seed_movie, seed_show_episode, test_state_with_source, write_stub};

use super::{credits, loudness, markers, storyboard, subtitles};

const SIZE: u64 = 64;
const TEXT_TRACK: &str = r#"[{"index":0,"codec":"subrip","language":"eng"}]"#;
const AUDIO_TRACK: &str = r#"[{"index":0,"codec":"aac","default":true}]"#;

fn url_answer(_: &std::path::Path) -> Value {
    json!({ "Ok": { "path": null, "url": "https://1.1.1.1/a.mkv", "size": SIZE } })
}

fn path_answer(held: &std::path::Path) -> Value {
    let copy = held.join("copy.mkv");
    std::fs::write(&copy, vec![0u8; SIZE as usize]).unwrap();
    json!({ "Ok": { "path": copy, "url": null, "size": SIZE } })
}

fn stubbed_movie(state: &SharedState, id: &str) -> String {
    seed_movie(state, id);
    let stub = write_stub(state, id, SIZE);
    let conn = state.db.get().unwrap();
    for sql in [
        format!("UPDATE items SET abs_path='{stub}', duration_ms=60000, subtitles='{TEXT_TRACK}' WHERE id='{id}'"),
        format!("UPDATE files SET abs_path='{stub}', probed=1, duration_ms=60000, subtitles='{TEXT_TRACK}', audio_tracks='{AUDIO_TRACK}' WHERE item_id='{id}'"),
    ] {
        conn.execute(&sql, []).unwrap();
    }
    stub
}

fn sig_of(subjects: &[(String, String)], id: &str) -> String {
    subjects.iter().find(|(subject, _)| subject == id).map(|(_, sig)| sig.clone()).expect("in scope")
}

#[test]
fn a_stub_with_no_copy_is_signed_unreadable_by_every_whole_file_stage_so_no_task_is_made() {
    let (state, server) = test_state_with_source(url_answer);
    let stub = stubbed_movie(&state, "m1");
    let answered = state.sources.find(&stub, false).expect("the source answered, it did not fail");
    assert!(matches!(answered.place, crate::services::source::Place::Upstream(_)), "no copy, by the source's own answer");

    assert_eq!(sig_of(&storyboard::enumerate(&state).unwrap(), "m1"), UNREADABLE_SIG);
    assert_eq!(sig_of(&credits::enumerate(&state).unwrap(), "m1"), UNREADABLE_SIG);
    assert_eq!(sig_of(&subtitles::enumerate(&state).unwrap(), "m1"), UNREADABLE_SIG);
    assert_eq!(sig_of(&loudness::enumerate(&state).unwrap(), "m1-f"), UNREADABLE_SIG);
    let asked: Vec<Value> = server.requests().iter().map(|r| serde_json::from_str(&r.body).unwrap()).collect();
    assert!(asked.iter().all(|body| body["warm"] == false), "a background stage never warms a cache: {asked:?}");
}

#[test]
fn a_deferred_stub_gets_no_ledger_row_and_nothing_retries() {
    let (state, _) = test_state_with_source(url_answer);
    stubbed_movie(&state, "m1");

    crate::db::pipeline::reconcile(&state.db, "storyboard", "item", &storyboard::enumerate(&state).unwrap(), 1).unwrap();

    let (pending, running, done, failed, blocked) = crate::db::pipeline::counts(&state.db, "storyboard").unwrap();
    assert_eq!((pending, running, done, failed, blocked), (0, 0, 0, 0, 0));
}

#[test]
fn a_stub_with_a_complete_copy_enters_every_stage_signed_by_the_stub_itself() {
    let (state, _) = test_state_with_source(path_answer);
    let stub = stubbed_movie(&state, "m1");
    let own = super::sig_for_path(&stub);

    for sig in [sig_of(&storyboard::enumerate(&state).unwrap(), "m1"), sig_of(&subtitles::enumerate(&state).unwrap(), "m1")] {
        assert!(sig.starts_with(&format!("{own}|")), "{sig}");
    }
    assert_ne!(sig_of(&credits::enumerate(&state).unwrap(), "m1"), UNREADABLE_SIG);
    assert_ne!(sig_of(&loudness::enumerate(&state).unwrap(), "m1-f"), UNREADABLE_SIG);
}

#[test]
fn a_stage_that_finds_the_copy_gone_fails_its_task_for_the_ledger_to_retry() {
    let (state, _) = test_state_with_source(url_answer);
    stubbed_movie(&state, "m1");
    let ctx = JobContext::for_test(state);

    for result in [
        storyboard::process(&ctx, "m1"),
        subtitles::process(&ctx, "m1"),
        loudness::process(&ctx, "m1-f"),
    ] {
        let error = result.expect_err("nothing to read yet").to_string();
        assert!(error.contains("no cached copy"), "{error}");
    }
}

fn season_with_a_stubbed_episode(
    answer: impl FnOnce(&std::path::Path) -> Value,
) -> (SharedState, String) {
    let (state, _) = test_state_with_source(answer);
    let (show, episode) = seed_show_episode(&state, "s1", "e1");
    let stub = write_stub(&state, "e1", SIZE);
    let conn = state.db.get().unwrap();
    for sql in [
        format!("UPDATE items SET abs_path='{stub}', duration_ms=1200000 WHERE id='{episode}'"),
        format!("UPDATE files SET abs_path='{stub}', duration_ms=1200000, probed=1 WHERE item_id='{episode}'"),
    ] {
        conn.execute(&sql, []).unwrap();
    }
    drop(conn);
    (state, show)
}

#[test]
fn an_episode_with_no_copy_is_left_out_of_its_season_until_it_has_one() {
    let (state, show) = season_with_a_stubbed_episode(url_answer);

    let subjects = markers::enumerate(&state).unwrap();

    assert!(subjects.iter().all(|(id, _)| !id.starts_with(&show)), "{subjects:?}");
}

#[test]
fn an_episode_with_a_copy_puts_its_season_in_scope() {
    let (state, show) = season_with_a_stubbed_episode(path_answer);

    let subjects = markers::enumerate(&state).unwrap();

    assert!(subjects.iter().any(|(id, sig)| id.starts_with(&show) && sig != UNREADABLE_SIG), "{subjects:?}");
}

#[test]
fn a_failed_task_runs_again_once_the_copy_was_evicted_and_cached_anew() {
    let (state, _) = test_state_with_source(path_answer);
    let stub = stubbed_movie(&state, "m1");
    let first = sig_of(&storyboard::enumerate(&state).unwrap(), "m1");
    crate::db::pipeline::reconcile(&state.db, "storyboard", "item", &[("m1".to_string(), first)], 1).unwrap();
    let conn = state.db.get().unwrap();
    conn.execute("UPDATE pipeline_tasks SET status='failed', attempts=5 WHERE subject_id='m1'", []).unwrap();
    drop(conn);

    let copy = state.config.data_dir.join("sources/tv.example.debrid/copy.mkv");
    std::fs::remove_file(&copy).unwrap();
    state.sources.forget(&stub);
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(&copy, vec![0u8; SIZE as usize]).unwrap();
    let second = sig_of(&storyboard::enumerate(&state).unwrap(), "m1");
    crate::db::pipeline::reconcile(&state.db, "storyboard", "item", &[("m1".to_string(), second)], 2).unwrap();

    let (pending, _, _, failed, _) = crate::db::pipeline::counts(&state.db, "storyboard").unwrap();
    assert_eq!((pending, failed), (1, 0), "the new copy resets the failed task");
}
