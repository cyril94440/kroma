use std::time::Duration;

use kroma_module_sdk::host::HostStorage as _;
use kroma_module_sdk::testing::blocking;
use serde_json::json;

use super::tests::{grab, host_with, release, releases, stored, Fake};
use super::*;
use crate::InstantAnswer;

fn seed_wanted(host: &kroma_module_sdk::host::testing::StubHost, status: &str) {
    let conn = host.db().get().unwrap();
    conn.execute(
        "INSERT INTO wanted (id, kind, tmdb_id, title, season, episode, status, updated_at) VALUES ('w1', 'movie', 438631, 'Dune', NULL, NULL, ?1, 1)",
        [status],
    )
    .unwrap();
}

fn wanted_status(host: &kroma_module_sdk::host::testing::StubHost) -> String {
    host.db()
        .get()
        .unwrap()
        .query_row("SELECT status FROM wanted WHERE id = 'w1'", [], |r| r.get(0))
        .unwrap()
}

fn peerless_hit() -> serde_json::Value {
    json!({ "Ok": { "peerless": true, "hit": { "key": "r0", "client_ref": "42" } } })
}

fn legacy_hit() -> serde_json::Value {
    json!({ "Ok": { "key": "r0", "client_ref": "42" } })
}

#[tokio::test]
async fn a_release_the_engine_cannot_settle_is_an_outcome_not_an_error_and_frees_everything() {
    let mut fake = Fake::answering(peerless_hit());
    fake.state = "error";
    let (host, manager) = host_with(&fake, &[]).await;
    seed_wanted(&host, "wanted");

    let outcome = grab(&host, &manager, releases(1)).await.unwrap();

    assert_eq!(outcome.status, InstantStatus::Failed);
    assert_eq!(outcome.key.as_deref(), Some("r0"));
    assert!(outcome.deterministic);
    assert!(outcome.download.is_none());
    assert_eq!(fake.engine.discarded.lock().unwrap().len(), 1);
    assert_eq!(wanted_status(&host), "wanted");
    let conn = manager.core().get().unwrap();
    assert!(db::active_downloads(&conn).unwrap().is_empty());
    let rows: Vec<String> = conn
        .prepare("SELECT status FROM downloads")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(rows, ["removed"], "never a failed row the give-up rule would count");
}

#[tokio::test]
async fn an_engine_that_never_finishes_in_the_budget_fails_the_release_without_blaming_it() {
    let mut fake = Fake::answering(peerless_hit());
    fake.state = "downloading";
    let (host, manager) = host_with(&fake, &[]).await;

    let (h, m) = (host.clone(), manager.clone());
    let outcome = blocking(move || m.grab_instant(&h, releases(1), Duration::from_millis(600)))
        .await
        .unwrap();

    assert_eq!(outcome.status, InstantStatus::Failed);
    assert!(!outcome.deterministic);
}

#[tokio::test]
async fn a_good_release_still_settles_into_a_completed_row() {
    let fake = Fake::answering(peerless_hit());
    let (host, manager) = host_with(&fake, &[]).await;
    seed_wanted(&host, "wanted");

    let outcome = grab(&host, &manager, releases(1)).await.unwrap();

    assert_eq!(outcome.status, InstantStatus::Grabbed);
    assert_eq!(stored(&manager, &outcome.download.unwrap().id).status, "completed");
    assert_eq!(wanted_status(&host), "grabbed");
}

#[tokio::test]
async fn a_closed_kill_switch_does_not_hold_an_engine_that_states_it_moves_no_peer_data() {
    let fake = Fake::answering(peerless_hit());
    let (host, manager) = host_with(&fake, &[]).await;
    manager.gate_open.store(false, std::sync::atomic::Ordering::Relaxed);

    let outcome = grab(&host, &manager, releases(1)).await.unwrap();

    assert_eq!(outcome.status, InstantStatus::Grabbed);
}

#[tokio::test]
async fn a_closed_kill_switch_holds_an_engine_that_does_not_say_so_and_asks_it_nothing_more() {
    let fake = Fake::answering(legacy_hit());
    let (host, manager) = host_with(&fake, &[]).await;
    manager.gate_open.store(false, std::sync::atomic::Ordering::Relaxed);

    let outcome = grab(&host, &manager, releases(1)).await.unwrap();

    assert_eq!(outcome.status, InstantStatus::Nothing);
    let asked = fake.engine.asked.lock().unwrap();
    assert_eq!(asked.len(), 1);
    assert!(asked[0]["args"]["candidates"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn an_engine_that_does_not_say_so_is_used_while_the_switch_is_open() {
    let fake = Fake::answering(legacy_hit());
    let (host, manager) = host_with(&fake, &[]).await;

    let outcome = grab(&host, &manager, releases(1)).await.unwrap();

    assert_eq!(outcome.status, InstantStatus::Grabbed);
}

#[tokio::test]
async fn the_callers_budget_travels_to_the_engine() {
    let fake = Fake::answering(json!({ "Ok": { "peerless": true, "hit": null } }));
    let (host, manager) = host_with(&fake, &[]).await;

    let (h, m) = (host.clone(), manager.clone());
    blocking(move || m.grab_instant(&h, vec![release("r0")], Duration::from_millis(7_500)))
        .await
        .unwrap();

    let asked = fake.engine.asked.lock().unwrap().last().unwrap().clone();
    assert_eq!(asked["args"]["budget_ms"], 7_500);
}

#[test]
fn the_answer_of_an_engine_is_read_in_the_stated_shape_and_the_legacy_ones() {
    let stated = InstantAnswer::from_json(json!({ "peerless": true, "hit": { "key": "a", "client_ref": "1" } })).unwrap();
    let bare = InstantAnswer::from_json(json!({ "key": "a", "client_ref": "1" })).unwrap();
    let none = InstantAnswer::from_json(json!(null)).unwrap();
    let empty = InstantAnswer::from_json(json!({ "peerless": true, "hit": null })).unwrap();

    assert!(stated.peerless && stated.hit.is_some());
    assert!(!bare.peerless && bare.hit.is_some());
    assert_eq!(none, InstantAnswer::default());
    assert!(empty.peerless && empty.hit.is_none());
}

#[tokio::test]
async fn the_capability_is_the_engines_own_answer_to_an_empty_list() {
    let (_, capable) = host_with(&Fake::answering(json!({ "Ok": null })), &[]).await;
    let (_, incapable) = host_with(&Fake::without_instant(), &[]).await;
    let (capable, incapable) = (capable, incapable);

    assert!(blocking(move || capable.instant_capable()).await);
    assert!(!blocking(move || incapable.instant_capable()).await);
}

#[test]
fn an_instant_release_tolerates_a_spec_missing_every_optional_field() {
    let parsed: InstantRelease = serde_json::from_value(json!({
        "key": "k", "spec": { "magnet_or_url": "magnet:?xt=urn:btih:a", "kind": "movie", "release_title": "R" }
    }))
    .unwrap();

    assert_eq!(parsed.spec.tmdb_id, None);
    assert!(parsed.spec.wanted_ids.is_empty());
}

#[test]
fn the_outcome_crosses_as_a_lowercase_status_with_the_key_and_the_row() {
    let none = serde_json::to_value(InstantOutcome::of(InstantStatus::Nothing)).unwrap();
    let unsupported = serde_json::to_value(InstantOutcome::of(InstantStatus::Unsupported)).unwrap();

    assert_eq!(
        none,
        json!({ "status": "nothing", "key": null, "download": null, "reason": null, "deterministic": false })
    );
    assert_eq!(unsupported["status"], "unsupported");
}

