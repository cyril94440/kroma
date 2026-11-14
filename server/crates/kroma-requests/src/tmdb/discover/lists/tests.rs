use serde_json::json;

use super::*;
use crate::testing::FakeTmdb;

fn page(rows: &[(u64, &str)], page: u32, total: u32) -> serde_json::Value {
    let results: Vec<_> = rows
        .iter()
        .map(|(id, title)| json!({ "id": id, "title": title, "name": title }))
        .collect();
    json!({ "page": page, "total_pages": total, "results": results })
}

#[test]
fn a_scoped_list_reads_its_own_endpoint_and_page() {
    let tmdb = FakeTmdb::start(|path| match path {
        "/movie/top_rated" => (200, page(&[(1, "A")], 2, 9)),
        _ => (404, json!({})),
    });

    let found = list("k", "en-US", DiscoverList::TopRated, DiscoverScope::Movies, 2).unwrap();

    assert_eq!(found.page, 2);
    assert_eq!(found.total_pages, 9);
    assert_eq!(found.hits[0].kind, RequestKind::Movie);
    assert!(tmdb.requests()[0].contains("page=2"));
}

#[test]
fn upcoming_shows_are_the_ones_on_the_air() {
    let _tmdb = FakeTmdb::start(|path| match path {
        "/tv/on_the_air" => (200, page(&[(5, "S")], 1, 1)),
        _ => (404, json!({})),
    });

    let found = list("k", "en-US", DiscoverList::Upcoming, DiscoverScope::Shows, 1).unwrap();

    assert_eq!(found.hits[0].kind, RequestKind::Show);
}

#[test]
fn a_mixed_list_interleaves_movies_and_shows() {
    let _tmdb = FakeTmdb::start(|path| match path {
        "/movie/popular" => (200, page(&[(1, "M1"), (2, "M2"), (3, "M3")], 1, 4)),
        "/tv/popular" => (200, page(&[(10, "S1")], 1, 7)),
        _ => (404, json!({})),
    });

    let found = list("k", "en-US", DiscoverList::Popular, DiscoverScope::All, 1).unwrap();

    let ids: Vec<u64> = found.hits.iter().map(|h| h.tmdb_id).collect();
    assert_eq!(ids, [1, 10, 2, 3]);
    assert_eq!(found.total_pages, 7);
}

#[test]
fn a_mixed_list_survives_one_side_failing() {
    let _tmdb = FakeTmdb::start(|path| match path {
        "/movie/popular" => (200, page(&[(1, "M1")], 1, 1)),
        _ => (500, json!({})),
    });

    let found = list("k", "en-US", DiscoverList::Popular, DiscoverScope::All, 1).unwrap();

    assert_eq!(found.hits.len(), 1);
}

#[test]
fn a_mixed_list_fails_when_both_sides_fail() {
    let _tmdb = FakeTmdb::start(|_| (500, json!({})));

    let found = list("k", "en-US", DiscoverList::Popular, DiscoverScope::All, 1);

    assert!(found.is_err());
}

#[test]
fn mixed_trending_is_one_call() {
    let tmdb = FakeTmdb::start(|path| match path {
        "/trending/all/week" => (
            200,
            json!({ "page": 1, "total_pages": 5, "results": [
                { "id": 1, "media_type": "movie", "title": "M" },
                { "id": 2, "media_type": "tv", "name": "S" },
                { "id": 3, "media_type": "person", "name": "P" }
            ]}),
        ),
        _ => (404, json!({})),
    });

    let found = list("k", "en-US", DiscoverList::Trending, DiscoverScope::All, 1).unwrap();

    assert_eq!(found.hits.len(), 2);
    assert_eq!(tmdb.requests().len(), 1);
}

#[test]
fn genres_come_back_named() {
    let _tmdb = FakeTmdb::start(|path| match path {
        "/genre/tv/list" => (200, json!({ "genres": [{ "id": 18, "name": "Drame" }] })),
        _ => (404, json!({})),
    });

    let found = genres("k", "fr-FR", RequestKind::Show).unwrap();

    assert_eq!(found, [Genre { id: 18, name: "Drame".into() }]);
}

#[test]
fn browsing_a_genre_filters_and_sorts_by_popularity() {
    let tmdb = FakeTmdb::start(|path| match path {
        "/discover/movie" => (200, page(&[(1, "A")], 3, 40)),
        _ => (404, json!({})),
    });

    let found = list_by_genre("k", "en-US", RequestKind::Movie, 28, 3).unwrap();

    let request = tmdb.requests()[0].clone();
    assert!(request.contains("with_genres=28"));
    assert!(request.contains("sort_by=popularity.desc"));
    assert!(request.contains("include_adult=false"));
    assert_eq!(found.total_pages, 40);
}

#[test]
fn list_names_round_trip() {
    for (name, expected) in [
        ("trending", DiscoverList::Trending),
        ("popular", DiscoverList::Popular),
        ("upcoming", DiscoverList::Upcoming),
        ("top-rated", DiscoverList::TopRated),
    ] {
        assert_eq!(DiscoverList::parse(name), Some(expected));
    }
    assert_eq!(DiscoverList::parse("nope"), None);
}
