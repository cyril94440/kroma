use std::time::Duration;

use serde_json::{json, Value};

use crate::fake_alldebrid::{Fake, Req, Resp};
use crate::it_engine::engine;
use crate::{Candidate, MAX_CANDIDATES};

pub(crate) const WIDE: Duration = Duration::from_secs(20);

pub(crate) fn candidate(n: usize) -> Candidate {
    Candidate {
        key: format!("rel-{n}"),
        magnet_or_url: format!("magnet:?xt=urn:btih:h{n}"),
    }
}

pub(crate) fn id_of(magnet: &str) -> u64 {
    1000 + magnet.rsplit('h').next().unwrap().parse::<u64>().unwrap()
}

pub(crate) fn upload_answer(req: &Req, ready: &[usize], refused: &[usize]) -> Resp {
    let entries: Vec<Value> = req
        .form_all("magnets[]")
        .iter()
        .map(|magnet| {
            let n = (id_of(magnet) - 1000) as usize;
            if refused.contains(&n) {
                json!({ "magnet": magnet, "error": { "code": "MAGNET_INVALID_URI", "message": "bad" } })
            } else {
                json!({ "magnet": magnet, "id": id_of(magnet), "ready": ready.contains(&n) })
            }
        })
        .collect();
    Resp::data(json!({ "magnets": entries }))
}

pub(crate) fn account(ready: &'static [usize], refused: &'static [usize]) -> Fake {
    account_holding(ready, refused, &[])
}

pub(crate) fn account_holding(
    ready: &'static [usize],
    refused: &'static [usize],
    holding: &'static [u64],
) -> Fake {
    Fake::start(move |req| match req.path.as_str() {
        "/v4.1/magnet/status" => {
            let magnets: Vec<Value> = holding
                .iter()
                .map(|id| json!({ "id": id, "filename": "mine", "size": 1, "status": "Ready", "statusCode": 4 }))
                .collect();
            Resp::data(json!({ "magnets": magnets }))
        }
        "/v4/magnet/upload" => upload_answer(req, ready, refused),
        "/v4/magnet/delete" => Resp::data(json!({ "message": "deleted" })),
        _ => Resp::failure("NOPE", "unexpected call"),
    })
}

pub(crate) fn deleted(fake: &Fake) -> Vec<String> {
    let mut ids: Vec<String> = fake
        .calls_to("/v4/magnet/delete")
        .iter()
        .map(|call| call.form()["id"].clone())
        .collect();
    ids.sort();
    ids
}

pub(crate) fn candidates(count: usize) -> Vec<Candidate> {
    (0..count).map(candidate).collect()
}

#[test]
fn the_best_ranked_ready_candidate_wins_and_every_other_upload_is_deleted() {
    let fake = account(&[1, 2], &[]);
    let dir = kroma_module_sdk::testing::temp_dir("ad-instant-best");

    let found = engine(&fake, dir.path())
        .instant(&candidates(4), WIDE)
        .unwrap()
        .unwrap();

    assert_eq!(
        (found.key.as_str(), found.client_ref.as_str()),
        ("rel-1", "1001")
    );
    assert_eq!(fake.calls_to("/v4/magnet/upload").len(), 1);
    assert_eq!(deleted(&fake), ["1000", "1002"]);
}

#[test]
fn a_lower_ranked_release_is_taken_when_it_is_the_only_one_ready() {
    let fake = account(&[3], &[]);
    let dir = kroma_module_sdk::testing::temp_dir("ad-instant-last");

    let found = engine(&fake, dir.path())
        .instant(&candidates(4), WIDE)
        .unwrap()
        .unwrap();

    assert_eq!(found.key, "rel-3");
    assert_eq!(deleted(&fake), ["1000", "1001", "1002"]);
}

#[test]
fn nothing_ready_answers_none_and_leaves_nothing_behind() {
    let fake = account(&[], &[]);
    let dir = kroma_module_sdk::testing::temp_dir("ad-instant-none");

    let found = engine(&fake, dir.path()).instant(&candidates(3), WIDE).unwrap();

    assert_eq!(found, None);
    assert_eq!(deleted(&fake), ["1000", "1001", "1002"]);
}

#[test]
fn a_refused_candidate_is_skipped_and_has_nothing_to_delete() {
    let fake = account(&[0, 2], &[0]);
    let dir = kroma_module_sdk::testing::temp_dir("ad-instant-refused");

    let found = engine(&fake, dir.path())
        .instant(&candidates(3), WIDE)
        .unwrap()
        .unwrap();

    assert_eq!(found.key, "rel-2");
    assert_eq!(deleted(&fake), ["1001"]);
}

#[test]
fn no_more_than_the_cap_is_ever_uploaded_and_never_more_than_a_batch_at_once() {
    let fake = account(&[], &[]);
    let dir = kroma_module_sdk::testing::temp_dir("ad-instant-cap");

    engine(&fake, dir.path())
        .instant(&candidates(MAX_CANDIDATES + 5), WIDE)
        .unwrap();

    let sizes: Vec<usize> = fake
        .calls_to("/v4/magnet/upload")
        .iter()
        .map(|call| call.form_all("magnets[]").len())
        .collect();
    assert_eq!(sizes, [3, 3, 2]);
    assert_eq!(deleted(&fake).len(), MAX_CANDIDATES);
}

#[test]
fn an_empty_list_is_a_capability_probe_that_never_reaches_the_network() {
    let fake = account(&[], &[]);
    let dir = kroma_module_sdk::testing::temp_dir("ad-instant-empty");

    assert_eq!(engine(&fake, dir.path()).instant(&[], WIDE).unwrap(), None);
    assert!(fake.calls().is_empty());
}

#[test]
fn a_delete_that_fails_is_not_fatal() {
    let fake = Fake::start(|req| match req.path.as_str() {
        "/v4.1/magnet/status" => Resp::data(json!({ "magnets": [] })),
        "/v4/magnet/upload" => upload_answer(req, &[0], &[]),
        _ => Resp::failure("MAGNET_PROCESSING", "busy"),
    });
    let dir = kroma_module_sdk::testing::temp_dir("ad-instant-delete-fails");

    let found = engine(&fake, dir.path())
        .instant(&candidates(3), WIDE)
        .unwrap()
        .unwrap();

    assert_eq!(found.key, "rel-0");
    assert_eq!(fake.calls_to("/v4/magnet/delete").len(), 2);
}

#[test]
fn the_account_is_listed_once_and_before_the_upload() {
    let fake = account(&[], &[]);
    let dir = kroma_module_sdk::testing::temp_dir("ad-instant-order");

    engine(&fake, dir.path()).instant(&candidates(2), WIDE).unwrap();

    let paths: Vec<String> = fake.calls().iter().map(|call| call.path.clone()).collect();
    assert_eq!(&paths[..2], ["/v4.1/magnet/status", "/v4/magnet/upload"]);
    assert_eq!(fake.calls_to("/v4.1/magnet/status").len(), 1);
    assert!(fake.calls_to("/v4.1/magnet/status")[0].query.is_empty());
}

#[test]
fn a_rejected_upload_batch_is_an_error_not_a_silent_none() {
    let fake = Fake::start(|req| match req.path.as_str() {
        "/v4.1/magnet/status" => Resp::data(json!({ "magnets": [] })),
        _ => Resp::failure("AUTH_BAD_APIKEY", "no"),
    });
    let dir = kroma_module_sdk::testing::temp_dir("ad-instant-auth");

    let error = engine(&fake, dir.path())
        .instant(&candidates(2), WIDE)
        .unwrap_err()
        .to_string();

    assert!(error.contains("rejected the API key"), "{error}");
}

#[test]
fn a_torrent_link_is_fetched_and_uploaded_as_a_file() {
    let torrent = Fake::start(|_| Resp::raw(200, b"d4:infod4:name1:xee".to_vec()));
    let fake = Fake::start(|req| match req.path.as_str() {
        "/v4.1/magnet/status" => Resp::data(json!({ "magnets": [] })),
        "/v4/magnet/upload/file" => {
            Resp::data(json!({ "files": [{ "file": "x.torrent", "id": 77, "ready": true }] }))
        }
        "/v4/magnet/delete" => Resp::data(json!({})),
        _ => Resp::failure("NOPE", "unexpected call"),
    });
    let dir = kroma_module_sdk::testing::temp_dir("ad-instant-link");
    let link = Candidate {
        key: "rel-link".into(),
        magnet_or_url: format!("{}/t.torrent", torrent.base),
    };

    let found = engine(&fake, dir.path()).instant(&[link], WIDE).unwrap().unwrap();

    assert_eq!(
        (found.key.as_str(), found.client_ref.as_str()),
        ("rel-link", "77")
    );
    assert!(fake.calls_to("/v4/magnet/delete").is_empty());
}
