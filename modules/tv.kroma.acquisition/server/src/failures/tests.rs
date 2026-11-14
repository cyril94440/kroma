use kroma_module_sdk::domain::RequestStatus;
use kroma_module_sdk::db::{self, WantedRow};
use kroma_module_sdk::host::testing::StubHost;

use super::*;

fn host() -> StubHost {
    let host = StubHost::with_core(db::testing::temp_pool("acq-failures"))
        .with_setting("acqStallHours", serde_json::json!(0));
    crate::rules::schema::install(host.store());
    host
}

fn grabbed(host: &StubHost, id: &str, episode: u32) {
    let row = WantedRow {
        id: id.into(),
        request_id: None,
        kind: "episode".into(),
        tmdb_id: 77,
        imdb_id: None,
        title: "Show".into(),
        year: None,
        season: Some(1),
        episode: Some(episode),
        air_date: None,
        status: "grabbed".into(),
        last_search_at: None,
    };
    db::insert_wanted(host.db(), &[row], 1).unwrap();
}

fn download(host: &StubHost, id: &str, episodes: &str, status: &str) {
    host.db()
        .get()
        .unwrap()
        .execute(
            "INSERT INTO downloads (id, client_id, client_ref, kind, tmdb_id, season, episodes, \
                 release_title, magnet_or_url, status, error, progress, grabbed_at) \
             VALUES (?1, 'embedded', '', 'episode', 77, 1, ?2, ?3, 'magnet:?x', ?4, 'tracker gone', 0.4, 1)",
            rusqlite::params![id, episodes, format!("Show.S01.{id}.1080p-GRP"), status],
        )
        .unwrap();
}

fn status_of(host: &StubHost, id: &str) -> String {
    let conn = host.db().get().unwrap();
    db::wanted_for_title(&conn, RequestKind::Show, 77)
        .unwrap()
        .into_iter()
        .find(|w| w.id == id)
        .unwrap()
        .status
}

#[test]
fn an_episode_whose_download_failed_is_missing_again_and_its_release_blocked() {
    let host = host();
    grabbed(&host, "w1", 1);
    download(&host, "d1", "[1]", "failed");

    let summary = failure_pass(&host, &|_| {}).unwrap();

    assert_eq!(status_of(&host, "w1"), "wanted");
    assert_eq!(summary.blocklisted, 1);
    let blocked = blocklist::for_title(host.store(), RequestKind::Show, 77).unwrap();
    assert_eq!(blocked[0].release_title, "Show.S01.d1.1080p-GRP");
    assert_eq!(blocked[0].reason, "tracker gone");
}

#[test]
fn a_failure_is_blocklisted_once() {
    let host = host();
    download(&host, "d1", "[1]", "failed");

    failure_pass(&host, &|_| {}).unwrap();
    let again = failure_pass(&host, &|_| {}).unwrap();

    assert_eq!(again.blocklisted, 0);
}

#[test]
fn an_episode_still_downloading_stays_grabbed() {
    let host = host();
    grabbed(&host, "w1", 1);
    download(&host, "d1", "[1]", "downloading");

    let summary = failure_pass(&host, &|_| {}).unwrap();

    assert_eq!(status_of(&host, "w1"), "grabbed");
    assert_eq!(summary.missing_again, 0);
}

#[test]
fn an_episode_whose_download_was_removed_is_missing_again() {
    let host = host();
    grabbed(&host, "w1", 1);
    grabbed(&host, "w2", 2);
    download(&host, "d1", "[2]", "downloading");

    failure_pass(&host, &|_| {}).unwrap();

    assert_eq!(status_of(&host, "w1"), "wanted");
    assert_eq!(status_of(&host, "w2"), "grabbed");
    assert!(
        host.jobs()
            .iter()
            .any(|(key, _)| *key == "acquisition.search")
    );
}

#[test]
fn a_season_pack_covers_every_episode_of_its_season() {
    let host = host();
    grabbed(&host, "w1", 1);
    grabbed(&host, "w2", 2);
    host.db()
        .get()
        .unwrap()
        .execute(
            "INSERT INTO downloads (id, client_id, client_ref, kind, tmdb_id, season, \
                 release_title, magnet_or_url, status, grabbed_at) \
             VALUES ('p1', 'embedded', '', 'season', 77, 1, 'Show.S01', 'magnet:?x', 'queued', 1)",
            [],
        )
        .unwrap();

    failure_pass(&host, &|_| {}).unwrap();

    assert_eq!(status_of(&host, "w1"), "grabbed");
    assert_eq!(status_of(&host, "w2"), "grabbed");
}

#[test]
fn a_download_that_has_not_moved_for_the_limit_has_stalled() {
    let d = Download {
        id: "d".into(),
        kind: "movie".into(),
        tmdb_id: 1,
        season: None,
        episodes: None,
        release_title: "M".into(),
        info_hash: None,
        indexer_id: None,
        status: "downloading".into(),
        error: None,
        progress: 0.4,
        grabbed_at: 0,
    };

    assert!(stalled(&d, Some(0), 72 * HOUR_MS, 72));
    assert!(!stalled(&d, Some(0), 71 * HOUR_MS, 72));
    assert!(!stalled(&d, Some(0), 1000 * HOUR_MS, 0));
    assert!(!stalled(
        &Download {
            status: "completed".into(),
            ..d
        },
        Some(0),
        1000 * HOUR_MS,
        72
    ));
}

#[test]
fn a_download_still_moving_is_not_given_up_on_however_long_ago_it_was_grabbed() {
    let d = Download {
        id: "d".into(),
        kind: "movie".into(),
        tmdb_id: 1,
        season: None,
        episodes: None,
        release_title: "M".into(),
        info_hash: None,
        indexer_id: None,
        status: "downloading".into(),
        error: None,
        progress: 0.9,
        grabbed_at: 0,
    };

    assert!(!stalled(&d, Some(1000 * HOUR_MS), 1001 * HOUR_MS, 72));
}

#[test]
fn a_download_grabbed_long_ago_but_still_moving_survives_the_pass() {
    let host = host().with_setting("acqStallHours", serde_json::json!(72));
    download(&host, "d1", "[1]", "downloading");

    let summary = failure_pass(&host, &|_| {}).unwrap();

    assert_eq!(summary.stalled, 0);
}

fn download_for(host: &StubHost, id: &str, request_id: &str, status: &str) {
    host.db()
        .get()
        .unwrap()
        .execute(
            "INSERT INTO downloads (id, client_id, client_ref, request_id, kind, tmdb_id, \
                 release_title, magnet_or_url, status, error, progress, grabbed_at) \
             VALUES (?1, 'embedded', '', ?2, 'movie', 88, ?1, 'magnet:?x', ?3, 'tracker gone', 0.1, 1)",
            rusqlite::params![id, request_id, status],
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
