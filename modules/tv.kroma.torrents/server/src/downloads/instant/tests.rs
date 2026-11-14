use std::sync::{Arc, Mutex};

use axum::routing::post;
use axum::{Json, Router};
use kroma_module_sdk::host::testing::StubHost;
use kroma_module_sdk::host::HostStorage as _;
use kroma_module_sdk::testing::{blocking, serve};
use serde_json::{json, Value};

use super::*;
use crate::db::test_support::client;
use crate::downloads::test_support::host_over;

const CLIENT: &str = "tv.kroma.torrents/client";

#[derive(Default)]
struct Engine {
    asked: Mutex<Vec<Value>>,
    adds: Mutex<u32>,
    removed: Mutex<Vec<String>>,
}

fn status_json(state: &str, progress: f64) -> Value {
    json!({
        "client_ref": "42", "name": "Dune", "info_hash": null, "progress": progress,
        "state": state, "down_bps": 0, "up_bps": 0, "peers": 0, "peers_seen": 0,
        "size_bytes": 10, "save_path": "/data/alldebrid/downloads/42",
        "files": ["Dune.mkv.kremote"], "error": null,
        "lifetime_downloaded_bytes": 0, "lifetime_uploaded_bytes": 0
    })
}

struct Fake {
    engine: Arc<Engine>,
    instant: Option<Value>,
    state: &'static str,
}

impl Fake {
    fn answering(instant: Value) -> Self {
        Self { engine: Arc::default(), instant: Some(instant), state: "completed" }
    }

    fn without_instant() -> Self {
        Self { engine: Arc::default(), instant: None, state: "completed" }
    }
}

async fn host_with(fake: &Fake, rows: &[(&str, &str)]) -> (StubHost, Arc<DownloadManager>) {
    let mut router = Router::new();
    let engine = fake.engine.clone();
    if let Some(answer) = fake.instant.clone() {
        let engine = engine.clone();
        router = router.route(
            "/_port/tv.kroma.torrents/client/instant",
            post(move |Json(body): Json<Value>| {
                let (engine, answer) = (engine.clone(), answer.clone());
                async move {
                    engine.asked.lock().unwrap().push(body);
                    Json(answer)
                }
            }),
        );
    }
    let counted = engine.clone();
    let state = fake.state;
    router = router
        .route(
            "/_port/tv.kroma.torrents/client/add",
            post(move |Json(_): Json<Value>| {
                let counted = counted.clone();
                async move {
                    *counted.adds.lock().unwrap() += 1;
                    Json(Ok::<_, String>("1".to_string()))
                }
            }),
        )
        .route(
            "/_port/tv.kroma.torrents/client/status",
            post(move |Json(_): Json<Value>| async move {
                Json(Some(status_json(state, if state == "completed" { 1.0 } else { 0.0 })))
            }),
        );
    let removing = engine.clone();
    router = router.route(
        "/_port/tv.kroma.torrents/client/remove",
        post(move |Json(body): Json<Value>| {
            let removing = removing.clone();
            async move {
                removing.removed.lock().unwrap().push(body["args"]["client_ref"].to_string());
                Json(Ok::<_, String>(()))
            }
        }),
    );
    let resolve = serve(router, ()).await;
    let (base, token) = resolve().expect("the fake engine is up");
    let host = host_over("torrents-instant", &[]).with_point(CLIENT, Some("fake"), &base, &token);
    let mut fake_client = client("fake1", 200, true, 5);
    fake_client.kind = "fake".into();
    crate::db::insert_download_client(host.store(), &fake_client).unwrap();
    for (id, url) in rows {
        let queued = db::DownloadRow {
            magnet_or_url: (*url).into(),
            ..db::test_support::download(id, "downloading", 1)
        };
        db::insert_download(host.db(), &queued).unwrap();
    }
    let manager = DownloadManager::new(
        Arc::new(host.clone()),
        host.data_dir(),
        host.db().clone(),
        host.store().clone(),
    );
    (host, manager)
}

fn release(key: &str) -> InstantRelease {
    InstantRelease {
        key: key.into(),
        spec: GrabSpec {
            magnet_or_url: format!("magnet:?xt=urn:btih:{key}"),
            kind: "movie".into(),
            tmdb_id: Some(438_631),
            title: Some("Dune".into()),
            release_title: format!("Dune.2021.{key}"),
            wanted_ids: vec!["w1".into()],
            ..GrabSpec::default()
        },
    }
}

fn releases(count: usize) -> Vec<InstantRelease> {
    (0..count).map(|n| release(&format!("r{n}"))).collect()
}

async fn grab(host: &StubHost, manager: &Arc<DownloadManager>, list: Vec<InstantRelease>) -> Result<InstantOutcome> {
    let (host, manager) = (host.clone(), manager.clone());
    blocking(move || manager.grab_instant(&host, list)).await
}

fn stored(manager: &DownloadManager, id: &str) -> db::DownloadRow {
    db::get_download(&manager.core().get().unwrap(), id).unwrap().unwrap()
}

#[tokio::test]
async fn the_chosen_release_becomes_a_completed_row_with_the_engines_reference_and_no_second_add() {
    let fake = Fake::answering(json!({ "Ok": { "key": "r1", "client_ref": "42" } }));
    let (host, manager) = host_with(&fake, &[]).await;

    let outcome = grab(&host, &manager, releases(3)).await.unwrap();

    assert_eq!(outcome.status, InstantStatus::Grabbed);
    assert_eq!(outcome.key.as_deref(), Some("r1"));
    let row = stored(&manager, &outcome.download.unwrap().id);
    assert_eq!(row.status, "completed");
    assert_eq!(row.client_ref, "42");
    assert_eq!(row.release_title, "Dune.2021.r1");
    assert_eq!(row.save_path.as_deref(), Some("/data/alldebrid/downloads/42"));
    assert_eq!(db::download_contents(manager.store(), &row.id).unwrap(), ["Dune.mkv.kremote"]);
    assert_eq!(*fake.engine.adds.lock().unwrap(), 0);
}

#[tokio::test]
async fn the_engine_is_asked_in_rank_order_with_each_releases_link() {
    let fake = Fake::answering(json!({ "Ok": null }));
    let (host, manager) = host_with(&fake, &[]).await;

    grab(&host, &manager, releases(3)).await.unwrap();

    let asked = fake.engine.asked.lock().unwrap()[0].clone();
    let keys: Vec<&str> = asked["args"]["candidates"].as_array().unwrap().iter().map(|c| c["key"].as_str().unwrap()).collect();
    assert_eq!(keys, ["r0", "r1", "r2"]);
    assert_eq!(asked["args"]["candidates"][0]["magnet_or_url"], "magnet:?xt=urn:btih:r0");
    assert_eq!(asked["client"]["kind"], "fake");
}

#[tokio::test]
async fn nothing_ready_leaves_no_row_behind() {
    let fake = Fake::answering(json!({ "Ok": null }));
    let (host, manager) = host_with(&fake, &[]).await;

    let outcome = grab(&host, &manager, releases(2)).await.unwrap();

    assert_eq!(outcome.status, InstantStatus::Nothing);
    assert!(db::active_downloads(&manager.core().get().unwrap()).unwrap().is_empty());
}

#[tokio::test]
async fn an_engine_without_the_method_is_unsupported_not_an_error() {
    let (host, manager) = host_with(&Fake::without_instant(), &[]).await;

    let outcome = grab(&host, &manager, releases(2)).await.unwrap();

    assert_eq!(outcome.status, InstantStatus::Unsupported);
}

#[tokio::test]
async fn the_embedded_engine_is_unsupported_without_any_call() {
    let host = host_over("torrents-instant-embedded", &[]);
    let manager = DownloadManager::new(Arc::new(host.clone()), host.data_dir(), host.db().clone(), host.store().clone());

    let outcome = grab(&host, &manager, releases(2)).await.unwrap();

    assert_eq!(outcome.status, InstantStatus::Unsupported);
}

#[tokio::test]
async fn a_closed_kill_switch_does_not_hold_an_engine_that_moves_no_peer_data() {
    let fake = Fake::answering(json!({ "Ok": { "key": "r0", "client_ref": "42" } }));
    let (host, manager) = host_with(&fake, &[]).await;
    manager.gate_open.store(false, std::sync::atomic::Ordering::Relaxed);

    let outcome = grab(&host, &manager, releases(1)).await.unwrap();

    assert_eq!(outcome.status, InstantStatus::Grabbed);
}

#[tokio::test]
async fn no_more_than_the_cap_is_offered_to_the_engine() {
    let fake = Fake::answering(json!({ "Ok": null }));
    let (host, manager) = host_with(&fake, &[]).await;

    grab(&host, &manager, releases(MAX_INSTANT_CANDIDATES + 4)).await.unwrap();

    let asked = fake.engine.asked.lock().unwrap()[0].clone();
    assert_eq!(asked["args"]["candidates"].as_array().unwrap().len(), MAX_INSTANT_CANDIDATES);
}

#[tokio::test]
async fn a_release_already_in_the_queue_is_not_offered_again() {
    let fake = Fake::answering(json!({ "Ok": null }));
    let (host, manager) = host_with(&fake, &[("d0", "magnet:?xt=urn:btih:r0")]).await;

    grab(&host, &manager, releases(2)).await.unwrap();

    let asked = fake.engine.asked.lock().unwrap()[0].clone();
    assert_eq!(asked["args"]["candidates"].as_array().unwrap().len(), 1);
    assert_eq!(asked["args"]["candidates"][0]["key"], "r1");
}

#[tokio::test]
async fn an_engine_that_fails_the_kept_release_fails_its_row_and_frees_the_magnet() {
    let mut fake = Fake::answering(json!({ "Ok": { "key": "r0", "client_ref": "42" } }));
    fake.state = "error";
    let (host, manager) = host_with(&fake, &[]).await;

    let error = grab(&host, &manager, releases(1)).await.unwrap_err();

    assert!(error.to_string().contains("engine"), "{error:#}");
    assert_eq!(fake.engine.removed.lock().unwrap().len(), 1);
    assert!(db::active_downloads(&manager.core().get().unwrap()).unwrap().is_empty());
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

    assert_eq!(none, json!({ "status": "nothing", "key": null, "download": null }));
    assert_eq!(unsupported["status"], "unsupported");
}
