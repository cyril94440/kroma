use kroma_module_sdk::db::{self, NewRequest};
use kroma_module_sdk::domain::{EpisodeRef, RequestKind, RequestStatus};
use kroma_module_sdk::host::testing::StubHost;

use super::*;
use crate::search::best::test_world::{monitor, monitored, want, wanted_row};

pub(super) fn open_movie_request(host: &StubHost) {
    let dune = monitored(RequestKind::Movie, 438_631, "Dune", Some(2021));
    monitor(host, &dune);
    db::insert_request(
        host.db(),
        &NewRequest {
            id: "r1".into(),
            kind: RequestKind::Movie,
            tmdb_id: 438_631,
            title: "Dune".into(),
            year: Some(2021),
            poster_url: None,
            seasons: None,
            episodes: None,
            status: RequestStatus::Approved,
            requested_by: None,
        },
        1,
    )
    .unwrap();
    let mut row = wanted_row("w1", &dune, None, None, "wanted");
    row.request_id = Some("r1".into());
    want(host, &[row]);
}

pub(super) fn open_episode_request(host: &StubHost) {
    let show = monitored(RequestKind::Show, 42, "The Show", Some(2020));
    monitor(host, &show);
    db::insert_request(
        host.db(),
        &NewRequest {
            id: "r2".into(),
            kind: RequestKind::Show,
            tmdb_id: 42,
            title: "The Show".into(),
            year: Some(2020),
            poster_url: None,
            seasons: None,
            episodes: Some(vec![
                EpisodeRef {
                    season: 1,
                    episode: 1,
                },
                EpisodeRef {
                    season: 1,
                    episode: 2,
                },
            ]),
            status: RequestStatus::Approved,
            requested_by: None,
        },
        1,
    )
    .unwrap();
    let mut row = wanted_row("w2", &show, Some(1), Some(1), "wanted");
    row.request_id = Some("r2".into());
    want(host, &[row]);
}

pub(super) fn open_second_episode(host: &StubHost) {
    let show = monitored(RequestKind::Show, 42, "The Show", Some(2020));
    let mut row = wanted_row("w3", &show, Some(1), Some(2), "wanted");
    row.request_id = Some("r2".into());
    want(host, &[row]);
}

pub(super) fn movie_ask() -> Ask {
    Ask {
        kind: "movie".into(),
        tmdb_id: 438_631,
        user_id: "u1".into(),
        ..Ask::default()
    }
}

pub(super) fn episode_ask() -> Ask {
    Ask {
        kind: "show".into(),
        tmdb_id: 42,
        season: Some(1),
        episode: Some(1),
        user_id: "u1".into(),
        ..Ask::default()
    }
}
