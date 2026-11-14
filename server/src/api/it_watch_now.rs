use std::sync::{Arc, Mutex};

use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use kroma_module_host::Contribution;
use kroma_requests::testing::FakeTmdb;
use serde_json::{json, Value};

use crate::api::test_support::{get, seed_session, send, test_app_with_contributions, TestApp};
use crate::model::Permission;

struct FakeAcquisition {
    base: String,
    asked: Arc<Mutex<Vec<Value>>>,
    reply: Arc<Mutex<Value>>,
}

async fn acquisition(capable: bool) -> FakeAcquisition {
    let asked: Arc<Mutex<Vec<Value>>> = Arc::default();
    let reply = Arc::new(Mutex::new(
        json!({ "Ok": { "status": "requested", "request_id": "r1" } }),
    ));
    let (log, answer) = (asked.clone(), reply.clone());
    let router = Router::new()
        .route(
            "/_port/acquisition/watch-now-capable",
            post(move |Json(_): Json<Value>| async move { Json(json!({ "Ok": capable })) }),
        )
        .route(
            "/_port/acquisition/watch-now",
            post(move |Json(body): Json<Value>| {
                let (log, answer) = (log.clone(), answer.clone());
                async move {
                    log.lock().unwrap().push(body);
                    Json(answer.lock().unwrap().clone())
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, router).await });
    FakeAcquisition { base, asked, reply }
}

fn app_over(fake: &FakeAcquisition) -> TestApp {
    let base = fake.base.clone();
    test_app_with_contributions(Arc::new(move |point| {
        if point != "acquisition" {
            return Vec::new();
        }
        vec![Contribution {
            module_id: "tv.kroma.acquisition".into(),
            instance: None,
            base_url: base.clone(),
            token: "port-token".into(),
        }]
    }))
}

fn user_with(t: &TestApp, name: &str, perms: &[Permission]) -> String {
    seed_session(&t.state, &format!("{name}@test.dev"), name, perms).1
}

const CREATE_AND_AUTO: [Permission; 3] = [
    Permission::Playback,
    Permission::RequestsCreate,
    Permission::RequestsAuto,
];
const CREATE_AND_MANAGE: [Permission; 3] = [
    Permission::Playback,
    Permission::RequestsCreate,
    Permission::RequestsManage,
];
const CREATE_ONLY: [Permission; 2] = [Permission::Playback, Permission::RequestsCreate];
const AUTO_ONLY: [Permission; 2] = [Permission::Playback, Permission::RequestsAuto];

async fn watch(t: &TestApp, token: &str, path: &str, body: Option<Value>) -> (StatusCode, Value) {
    send(&t.app, "POST", path, Some(token), body).await
}

#[tokio::test]
async fn a_requester_who_is_auto_approved_or_a_moderator_is_answered() {
    let fake = acquisition(true).await;
    let t = app_over(&fake);
    let auto = user_with(&t, "auto", &CREATE_AND_AUTO);
    let manager = user_with(&t, "manager", &CREATE_AND_MANAGE);

    for token in [auto, manager] {
        let (status, body) = watch(&t, &token, "/api/discover/movie/603/watch-now", None).await;

        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body, json!({ "status": "requested", "requestId": "r1" }));
    }
}

#[tokio::test]
async fn nobody_else_learns_the_route_exists() {
    let fake = acquisition(true).await;
    let t = app_over(&fake);
    let plain = user_with(&t, "plain", &CREATE_ONLY);
    let no_create = user_with(&t, "nocreate", &AUTO_ONLY);

    for token in [plain, no_create] {
        let (status, _) = watch(&t, &token, "/api/discover/movie/603/watch-now", None).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
    }
    assert!(
        fake.asked.lock().unwrap().is_empty(),
        "the module was never asked"
    );
}

#[tokio::test]
async fn without_the_acquisition_module_the_route_is_not_found_for_everyone() {
    let t = test_app_with_contributions(Arc::new(|_| Vec::new()));

    let (status, _) = watch(&t, &t.token, "/api/discover/movie/603/watch-now", None).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_show_asks_the_module_for_the_episode_the_caller_named() {
    let fake = acquisition(true).await;
    let t = app_over(&fake);

    let (status, _) = watch(
        &t,
        &t.token,
        "/api/discover/tv/1396/watch-now",
        Some(json!({ "season": 2, "episode": 5 })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let asked = fake.asked.lock().unwrap()[0].clone();
    assert_eq!(asked["kind"], "show");
    assert_eq!(
        (asked["season"].clone(), asked["episode"].clone()),
        (json!(2), json!(5))
    );
    assert_eq!(asked["user_id"], t.user_id);
    assert_eq!(asked["exclude"], json!([]));
}

#[tokio::test]
async fn a_title_held_in_a_library_the_viewer_cannot_see_is_answered_requested_not_imported_again() {
    let fake = acquisition(true).await;
    *fake.reply.lock().unwrap() = json!({ "Ok": { "status": "present", "request_id": null } });
    let t = app_over(&fake);

    let (status, body) = watch(&t, &t.token, "/api/discover/movie/603/watch-now", None).await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "status": "requested", "requestId": null }));
}

#[tokio::test]
async fn a_show_with_one_number_is_a_bad_request_with_a_translated_message() {
    let fake = acquisition(true).await;
    let t = app_over(&fake);

    let status = watch(
        &t,
        &t.token,
        "/api/discover/tv/1396/watch-now",
        Some(json!({ "season": 2 })),
    )
    .await;

    assert_eq!(status.0, StatusCode::BAD_REQUEST);
    assert_eq!(status.1["code"], "error.watchNow.episodeRequired");
}

#[tokio::test]
async fn an_unavailable_title_says_why_in_the_readers_language() {
    let fake = acquisition(true).await;
    *fake.reply.lock().unwrap() =
        json!({ "Ok": { "status": "unavailable", "reason": "noRelease", "request_id": "r9" } });
    let t = app_over(&fake);

    let (status, body) = watch(&t, &t.token, "/api/discover/movie/603/watch-now", None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "unavailable");
    assert_eq!(body["reason"], "noRelease");
    assert_eq!(body["requestId"], "r9");
    assert!(
        body["message"]
            .as_str()
            .is_some_and(|m| !m.is_empty() && !m.starts_with("error.")),
        "{body}"
    );
}

#[tokio::test]
async fn a_module_that_fails_answers_a_bad_gateway_with_a_translated_message() {
    let fake = acquisition(true).await;
    *fake.reply.lock().unwrap() = json!({ "Err": "boom" });
    let t = app_over(&fake);

    let (status, body) = watch(&t, &t.token, "/api/discover/movie/603/watch-now", None).await;

    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(body["code"], "error.watchNow.failed");
}

#[tokio::test]
async fn a_title_the_library_already_plays_is_ready_without_asking_the_module() {
    let fake = acquisition(true).await;
    let t = app_over(&fake);
    let item = crate::api::test_support::seed_playable_item(&t.state, "Held", b"x");
    let abs: String = t
        .state
        .db
        .get()
        .unwrap()
        .query_row(
            "SELECT abs_path FROM files WHERE item_id = ?1",
            [&item],
            |r| r.get(0),
        )
        .unwrap();
    crate::db::set_file_tmdb(&t.state.db, &abs, 603).unwrap();

    let (status, body) = watch(&t, &t.token, "/api/discover/movie/603/watch-now", None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ready");
    assert_eq!(body["itemId"], item);
    assert!(fake.asked.lock().unwrap().is_empty());
}

fn tmdb() -> FakeTmdb {
    FakeTmdb::start_shared(|path| match path {
        "/movie/603" => (
            200,
            json!({ "title": "The Matrix", "release_date": "1999-03-31", "runtime": 136, "genres": [] }),
        ),
        _ => (404, json!({})),
    })
}

#[tokio::test]
async fn the_detail_offers_watch_now_only_to_a_viewer_who_may_use_it_on_a_server_that_can() {
    let _tmdb = tmdb();
    let fake = acquisition(true).await;
    let t = app_over(&fake);
    let plain = user_with(&t, "plain", &CREATE_ONLY);
    let auto = user_with(&t, "auto", &CREATE_AND_AUTO);

    let (_, plain_view) = get(&t.app, "/api/discover/movie/603", Some(&plain)).await;
    let (_, auto_view) = get(&t.app, "/api/discover/movie/603", Some(&auto)).await;

    assert_eq!(plain_view["watchNow"], false, "{plain_view}");
    assert_eq!(auto_view["watchNow"], true, "{auto_view}");
}

#[tokio::test]
async fn the_detail_never_offers_it_when_the_download_client_cannot_fetch_at_once() {
    let _tmdb = tmdb();
    let fake = acquisition(false).await;
    let t = app_over(&fake);

    let (_, view) = get(&t.app, "/api/discover/movie/603", Some(&t.token)).await;

    assert_eq!(view["watchNow"], false);
}

#[tokio::test]
async fn the_detail_never_offers_it_for_a_title_the_library_holds() {
    let _tmdb = tmdb();
    let fake = acquisition(true).await;
    let t = app_over(&fake);
    let item = crate::api::test_support::seed_playable_item(&t.state, "Held", b"x");
    let abs: String = t
        .state
        .db
        .get()
        .unwrap()
        .query_row(
            "SELECT abs_path FROM files WHERE item_id = ?1",
            [&item],
            |r| r.get(0),
        )
        .unwrap();
    crate::db::set_file_tmdb(&t.state.db, &abs, 603).unwrap();
    t.state.db.get().unwrap().execute(
        "INSERT INTO metadata_core (subject_kind, subject_id, tmdb_id, updated_at) VALUES ('item', ?1, 603, 0)",
        [&item],
    ).unwrap();

    let (_, view) = get(&t.app, "/api/discover/movie/603", Some(&t.token)).await;

    assert_eq!(view["inLibrary"], true, "{view}");
    assert_eq!(view["watchNow"], false);
}

#[tokio::test]
async fn the_detail_never_offers_it_without_the_acquisition_module() {
    let _tmdb = tmdb();
    let t = test_app_with_contributions(Arc::new(|_| Vec::new()));

    let (_, view) = get(&t.app, "/api/discover/movie/603", Some(&t.token)).await;

    assert_eq!(view["watchNow"], false);
}
