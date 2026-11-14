use kroma_module_sdk::domain::RequestStatus;
use kroma_module_sdk::db;
use kroma_module_sdk::host::testing::StubHost;

use super::tests::host;
use super::*;

fn download_for(host: &StubHost, id: &str, request_id: &str, status: &str) {
    host.db()
        .get()
        .unwrap()
        .execute(
            "INSERT INTO downloads (id, client_id, client_ref, request_id, kind, tmdb_id, \
                 release_title, magnet_or_url, status, error, progress, grabbed_at) \
             VALUES (?1, 'embedded', '', ?2, 'movie', 88, ?1, 'magnet:?x', ?3, 'tracker gone', 0.1, ?4)",
            rusqlite::params![id, request_id, status, kroma_module_sdk::primitives::now_ms()],
        )
        .unwrap();
}

fn request(host: &StubHost, id: &str, status: RequestStatus) {
    kroma_module_sdk::db::insert_request(
        host.db(),
        &kroma_module_sdk::db::NewRequest {
            id: id.into(),
            kind: RequestKind::Movie,
            tmdb_id: 88,
            title: "Film".into(),
            year: None,
            poster_url: None,
            seasons: None,
            episodes: None,
            status,
            requested_by: None,
        },
        1,
    )
    .unwrap();
}

fn request_status(host: &StubHost, id: &str) -> RequestStatus {
    let conn = host.db().get().unwrap();
    db::get_request(&conn, id).unwrap().unwrap().status
}

#[test]
fn a_request_whose_third_download_failed_is_marked_failed() {
    let host = host();
    request(&host, "r1", RequestStatus::Approved);
    for id in ["d1", "d2", "d3"] {
        download_for(&host, id, "r1", "failed");
    }

    let summary = failure_pass(&host, &|_| {}).unwrap();

    assert_eq!(summary.requests_failed, 1);
    assert_eq!(request_status(&host, "r1"), RequestStatus::Failed);
}

#[test]
fn a_request_with_a_download_still_live_keeps_trying() {
    let host = host();
    request(&host, "r1", RequestStatus::Approved);
    for id in ["d1", "d2", "d3"] {
        download_for(&host, id, "r1", "failed");
    }
    download_for(&host, "d4", "r1", "downloading");

    let summary = failure_pass(&host, &|_| {}).unwrap();

    assert_eq!(summary.requests_failed, 0);
    assert_eq!(request_status(&host, "r1"), RequestStatus::Approved);
}

#[test]
fn two_failures_are_not_enough_to_give_up() {
    let host = host();
    request(&host, "r1", RequestStatus::Approved);
    for id in ["d1", "d2"] {
        download_for(&host, id, "r1", "failed");
    }

    let summary = failure_pass(&host, &|_| {}).unwrap();

    assert_eq!(summary.requests_failed, 0);
}

#[test]
fn a_request_already_failed_is_not_failed_again_on_the_next_pass() {
    let host = host();
    request(&host, "r1", RequestStatus::Approved);
    for id in ["d1", "d2", "d3"] {
        download_for(&host, id, "r1", "failed");
    }
    failure_pass(&host, &|_| {}).unwrap();

    let again = failure_pass(&host, &|_| {}).unwrap();

    assert_eq!(again.requests_failed, 0);
}

fn age(host: &StubHost, id: &str, grabbed_at: i64) {
    host.db()
        .get()
        .unwrap()
        .execute("UPDATE downloads SET grabbed_at = ?2 WHERE id = ?1", rusqlite::params![id, grabbed_at])
        .unwrap();
}

#[test]
fn failures_older_than_the_first_run_of_the_rule_are_never_counted() {
    let host = host();
    request(&host, "r1", RequestStatus::Approved);
    for id in ["d1", "d2", "d3"] {
        download_for(&host, id, "r1", "failed");
        age(&host, id, 5);
    }

    let summary = failure_pass(&host, &|_| {}).unwrap();

    assert_eq!(summary.requests_failed, 0);
    assert_eq!(request_status(&host, "r1"), RequestStatus::Approved);
}

#[test]
fn failures_before_the_requests_last_approval_are_not_counted() {
    let host = host();
    request(&host, "r1", RequestStatus::Approved);
    host.store()
        .get()
        .unwrap()
        .execute("UPDATE rule_settings SET value = '0' WHERE key = 'give_up_since'", [])
        .unwrap();
    for id in ["d1", "d2", "d3"] {
        download_for(&host, id, "r1", "failed");
        age(&host, id, 10);
    }
    host.db()
        .get()
        .unwrap()
        .execute("UPDATE requests SET updated_at = 1000", [])
        .unwrap();

    let summary = failure_pass(&host, &|_| {}).unwrap();

    assert_eq!(summary.requests_failed, 0);
}

#[test]
fn releases_a_watch_now_turned_down_never_count_toward_giving_up() {
    let host = host();
    request(&host, "r1", RequestStatus::Approved);
    for id in ["d1", "d2", "d3", "d4"] {
        download_for(&host, id, "r1", "removed");
    }

    let summary = failure_pass(&host, &|_| {}).unwrap();

    assert_eq!(summary.requests_failed, 0);
}
