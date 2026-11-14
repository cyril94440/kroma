use std::time::Duration;

use kroma_module_sdk::host::testing::StubHost;
use kroma_module_sdk::host::HostCtx;
use serde_json::{json, Value};

use crate::api::{is_retry_later, Api, Delayed, RetryLater};
use crate::fake_alldebrid::{Fake, Resp, Script};
use crate::it_engine::engine;
use crate::it_instant::{account, candidates, deleted, upload_answer, WIDE};
use crate::it_ready::node;
use crate::port::routes;
use crate::{Candidate, TorrentState};

#[test]
fn a_429_with_an_html_body_is_retry_later_and_the_next_call_does_not_reach_the_server() {
    let fake = Fake::start(|_| Resp::raw(429, b"<html>slow down</html>".to_vec()));
    let api = Api::new(&fake.base, "k").unwrap();

    let first = api.user().unwrap_err();
    let second = api.user().unwrap_err();

    assert!(first.downcast_ref::<RetryLater>().is_some_and(|r| r.status == 429));
    assert!(is_retry_later(&second));
    assert_eq!(fake.calls().len(), 1);
}

#[test]
fn a_503_is_retry_later_too_and_a_good_answer_clears_the_pause() {
    let fake = Fake::start(|_| Resp::raw(503, Vec::new()));
    let api = Api::new(&fake.base, "k").unwrap();
    assert!(is_retry_later(&api.user().unwrap_err()));

    let healthy = Fake::start(|_| Resp::data(json!({ "user": { "username": "n", "isPremium": true } })));
    let other = Api::new(&healthy.base, "k").unwrap();
    assert_eq!(other.user().unwrap().username, "n");
}

#[test]
fn magnet_status_and_unlock_are_posted_as_the_documentation_shows() {
    let fake = Fake::start(|req| match req.path.as_str() {
        "/v4.1/magnet/status" => Resp::data(json!({ "magnets": { "id": 7, "statusCode": 4 } })),
        _ => Resp::data(json!({ "link": "http://x/y", "filesize": 1 })),
    });
    let api = Api::new(&fake.base, "k").unwrap();

    let magnet = api.magnet("7").unwrap().unwrap();
    api.unlock("https://alldebrid.com/f/a").unwrap();

    assert_eq!(magnet.status_code, 4);
    let methods: Vec<String> = fake.calls().iter().map(|call| call.method.clone()).collect();
    assert_eq!(methods, ["POST", "POST"]);
    assert_eq!(fake.calls()[0].form()["id"], "7");
}

#[test]
fn a_delayed_link_is_refused_with_its_own_clear_error() {
    let fake = Fake::start(|_| Resp::data(json!({ "link": "", "delayed": 2277564 })));
    let api = Api::new(&fake.base, "k").unwrap();

    let error = api.unlock("https://alldebrid.com/f/a").unwrap_err();

    assert_eq!(error.downcast_ref::<Delayed>().unwrap().id, "2277564");
    assert!(error.to_string().contains("delayed link"), "{error}");
}

#[test]
fn the_account_listing_accepts_a_single_magnet_object_as_well_as_an_array() {
    let fake = Fake::start(|_| Resp::data(json!({ "magnets": { "id": 9 } })));
    let api = Api::new(&fake.base, "k").unwrap();

    assert!(api.magnet_ids().unwrap().contains("9"));
}

#[test]
fn a_full_account_stops_the_uploads_after_the_first_batch() {
    let fake = Fake::start(|req| match req.path.as_str() {
        "/v4.1/magnet/status" => Resp::data(json!({ "magnets": [] })),
        "/v4/magnet/upload" => {
            let entries: Vec<Value> = req
                .form_all("magnets[]")
                .iter()
                .map(|magnet| json!({ "magnet": magnet, "error": { "code": "MAGNET_TOO_MANY_ACTIVE", "message": "30" } }))
                .collect();
            Resp::data(json!({ "magnets": entries }))
        }
        _ => Resp::failure("NOPE", "unexpected call"),
    });
    let dir = kroma_module_sdk::testing::temp_dir("ad-instant-full");

    let found = engine(&fake, dir.path()).instant(&candidates(8), WIDE).unwrap();

    assert_eq!(found, None);
    assert_eq!(fake.calls_to("/v4/magnet/upload").len(), 1);
}

#[test]
fn a_ready_magnet_means_no_torrent_file_is_fetched_or_uploaded() {
    let torrent = Fake::start(|_| Resp::raw(200, b"d4:infod4:name1:xee".to_vec()));
    let fake = account(&[0], &[]);
    let dir = kroma_module_sdk::testing::temp_dir("ad-instant-magnet-first");
    let mut ranked = candidates(2);
    ranked.insert(
        0,
        Candidate {
            key: "link".into(),
            magnet_or_url: format!("{}/t.torrent", torrent.base),
        },
    );

    let found = engine(&fake, dir.path()).instant(&ranked, WIDE).unwrap().unwrap();

    assert_eq!(found.key, "rel-0");
    assert!(torrent.calls().is_empty());
    assert!(fake.calls_to("/v4/magnet/upload/file").is_empty());
}

#[test]
fn a_spent_budget_uploads_nothing() {
    let fake = account(&[0], &[]);
    let dir = kroma_module_sdk::testing::temp_dir("ad-instant-budget");

    let found = engine(&fake, dir.path())
        .instant(&candidates(3), Duration::ZERO)
        .unwrap();

    assert_eq!(found, None);
    assert!(fake.calls_to("/v4/magnet/upload").is_empty());
}

#[test]
fn a_rate_limited_upload_is_an_error_the_caller_can_retry_not_a_dead_source() {
    let fake = Fake::start(|req| match req.path.as_str() {
        "/v4.1/magnet/status" => Resp::data(json!({ "magnets": [] })),
        "/v4/magnet/upload" => Resp::raw(429, Vec::new()),
        _ => upload_answer(req, &[], &[]),
    });
    let dir = kroma_module_sdk::testing::temp_dir("ad-instant-429");

    let error = engine(&fake, dir.path()).instant(&candidates(2), WIDE).unwrap_err();

    assert!(is_retry_later(&error), "{error:#}");
    assert!(deleted(&fake).is_empty());
}

fn deterministic_failure(tree: Value) -> Fake {
    let mut script = Script::new(tree, vec![], true);
    script.status_code = std::sync::Arc::new(std::sync::atomic::AtomicU8::new(4));
    script.start()
}

#[test]
fn a_magnet_with_nothing_playable_is_an_error_state_not_a_transport_failure() {
    let fake = deterministic_failure(json!([node("disc.iso", 9), node("part.rar", 9)]));
    let dir = kroma_module_sdk::testing::temp_dir("ad-plan-novideo");

    let status = engine(&fake, dir.path()).status("7").unwrap().unwrap();

    assert_eq!(status.state, TorrentState::Error);
    assert!(status.error.unwrap().contains("no video"));
}

#[test]
fn a_magnet_whose_files_all_lack_a_link_is_an_error_state() {
    let fake = deterministic_failure(json!([{ "n": "a.mkv", "s": 5 }]));
    let dir = kroma_module_sdk::testing::temp_dir("ad-plan-nolink");

    let status = engine(&fake, dir.path()).status("7").unwrap().unwrap();

    assert_eq!(status.state, TorrentState::Error);
    assert!(status.error.unwrap().contains("no file with a download link"));
}

#[test]
fn a_per_entry_error_in_the_file_listing_is_an_error_state() {
    let fake = Fake::start(|req| match req.path.as_str() {
        "/v4.1/magnet/status" => Resp::data(json!({ "magnets": [{ "id": 7, "statusCode": 4, "filename": "x", "size": 1 }] })),
        "/v4/magnet/files" => Resp::data(json!({ "magnets": [{ "id": "7", "error": { "code": "MAGNET_INVALID_ID", "message": "bad" } }] })),
        _ => Resp::failure("NOPE", "x"),
    });
    let dir = kroma_module_sdk::testing::temp_dir("ad-plan-entry-error");

    let status = engine(&fake, dir.path()).status("7").unwrap().unwrap();

    assert_eq!(status.state, TorrentState::Error);
}

#[test]
fn a_rejected_key_while_planning_stays_a_failure_for_the_caller_to_retry() {
    let fake = Fake::start(|req| match req.path.as_str() {
        "/v4.1/magnet/status" => Resp::data(json!({ "magnets": [{ "id": 7, "statusCode": 4, "filename": "x", "size": 1 }] })),
        _ => Resp::failure("AUTH_BAD_APIKEY", "no"),
    });
    let dir = kroma_module_sdk::testing::temp_dir("ad-plan-auth");

    let error = engine(&fake, dir.path()).status("7").unwrap_err();

    assert!(error.to_string().contains("rejected the API key"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_warm_locate_never_asks_the_account_endpoint() {
    let body: Vec<u8> = (0..1000).map(|i| (i % 251) as u8).collect();
    let fake = Script::new(json!([node("Movie.mkv", 1000)]), vec![("Movie.mkv", body)], true).start();
    let host = StubHost::new();
    engine(&fake, &host.data_dir().join("alldebrid")).status("7").unwrap();
    let app = routes::<StubHost>().with_state(host.clone());

    for _ in 0..3 {
        let req = axum::http::Request::builder()
            .method("POST")
            .uri("/_port/source/locate")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(json!({ "ref": "7:Movie.mkv", "warm": true }).to_string()))
            .unwrap();
        tower::ServiceExt::oneshot(app.clone(), req).await.unwrap();
    }

    assert!(fake.calls_to("/v4/user").is_empty());
}
