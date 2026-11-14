use axum::http::StatusCode;
use serde_json::{json, Value};

use kroma_requests::testing::FakeTmdb;

use crate::api::test_support::{get, seed_session, test_app_with_tmdb, TestApp};
use crate::db;
use crate::model::{Permission, RequestKind, RequestStatus};

const SINTEL: u64 = 45745;

fn page(rows: &[(u64, &str)], page: u32, total: u32) -> Value {
    let results: Vec<Value> = rows
        .iter()
        .map(|(id, title)| json!({ "id": id, "title": title, "name": title, "release_date": "2010-09-30" }))
        .collect();
    json!({ "page": page, "total_pages": total, "results": results })
}

fn tmdb() -> FakeTmdb {
    FakeTmdb::start_shared(|path| match path {
        "/movie/popular" => (200, page(&[(SINTEL, "Sintel"), (7, "Other")], 1, 12)),
        "/movie/upcoming" => (200, page(&[(8, "Soon")], 1, 3)),
        "/tv/on_the_air" => (200, page(&[(9, "Airing")], 1, 2)),
        "/movie/top_rated" => (200, page(&[(10, "Best")], 2, 30)),
        "/genre/movie/list" => (200, json!({ "genres": [{ "id": 28, "name": "Action" }] })),
        "/discover/movie" => (200, page(&[(11, "Punchy")], 1, 5)),
        _ => (404, json!({})),
    })
}

fn titles(body: &Value) -> Vec<&str> {
    body["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["title"].as_str().unwrap())
        .collect()
}

fn request_sintel(t: &TestApp) {
    let request = db::NewRequest {
        id: "r1".into(),
        kind: RequestKind::Movie,
        tmdb_id: SINTEL,
        title: "Sintel".into(),
        year: Some(2010),
        poster_url: None,
        seasons: None,
        episodes: None,
        status: RequestStatus::Pending,
        requested_by: Some(t.user_id.clone()),
    };
    db::insert_request(&t.state.db, &request, 1).unwrap();
}

#[tokio::test]
async fn a_popular_list_flags_each_title_against_the_open_requests() {
    let _tmdb = tmdb();
    let t = test_app_with_tmdb();
    request_sintel(&t);

    let (status, body) = get(&t.app, "/api/discover/popular?type=movie", Some(&t.token)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(titles(&body), ["Sintel", "Other"]);
    assert_eq!(body["totalPages"], 12);
    assert_eq!(body["results"][0]["requestStatus"], "pending");
    assert_eq!(body["results"][0]["requestId"], "r1");
    assert_eq!(body["results"][1]["requestId"], Value::Null);
}

#[tokio::test]
async fn upcoming_movies_and_shows_are_merged_without_a_type() {
    let _tmdb = tmdb();
    let t = test_app_with_tmdb();

    let (status, body) = get(&t.app, "/api/discover/upcoming", Some(&t.token)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(titles(&body), ["Soon", "Airing"]);
    assert_eq!(body["results"][1]["kind"], "show");
}

#[tokio::test]
async fn a_list_pages_through_the_page_the_caller_asks_for() {
    let tmdb = tmdb();
    let t = test_app_with_tmdb();

    let (_, body) = get(&t.app, "/api/discover/top-rated?type=movie&page=2", Some(&t.token)).await;

    assert_eq!(body["page"], 2);
    assert_eq!(body["totalPages"], 30);
    assert!(tmdb.requests().iter().any(|r| r.contains("page=2")));
}

#[tokio::test]
async fn an_unknown_list_is_not_found() {
    let _tmdb = tmdb();
    let t = test_app_with_tmdb();

    let (status, _) = get(&t.app, "/api/discover/nonsense", Some(&t.token)).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_genres_of_a_kind_are_listed() {
    let _tmdb = tmdb();
    let t = test_app_with_tmdb();

    let (status, body) = get(&t.app, "/api/discover/genres?type=movie", Some(&t.token)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({ "genres": [{ "id": 28, "name": "Action" }] }));
}

#[tokio::test]
async fn genres_need_a_kind() {
    let _tmdb = tmdb();
    let t = test_app_with_tmdb();

    let (status, _) = get(&t.app, "/api/discover/genres", Some(&t.token)).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn browsing_a_genre_lists_its_titles_flagged() {
    let tmdb = tmdb();
    let t = test_app_with_tmdb();

    let (status, body) = get(&t.app, "/api/discover/genre/28?type=movie", Some(&t.token)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(titles(&body), ["Punchy"]);
    assert_eq!(body["results"][0]["inLibrary"], false);
    assert!(tmdb.requests().iter().any(|r| r.contains("with_genres=28")));
}

#[tokio::test]
async fn every_list_needs_the_request_permission() {
    let _tmdb = tmdb();
    let t = test_app_with_tmdb();
    let (_, viewer) = seed_session(&t.state, "viewer@test.dev", "viewer", &[Permission::Playback]);

    for uri in [
        "/api/discover/popular",
        "/api/discover/upcoming",
        "/api/discover/top-rated",
        "/api/discover/genres?type=movie",
        "/api/discover/genre/28?type=movie",
    ] {
        let (status, _) = get(&t.app, uri, Some(&viewer)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{uri}");
    }
}
