use std::path::Path;

use kroma_module_sdk::db;
use kroma_module_sdk::domain::{RequestKind, RequestStatus};
use kroma_module_sdk::testing::blocking;
use serde_json::json;

use super::test_world::*;
use super::*;

#[tokio::test]
async fn a_movie_the_client_holds_is_imported_at_once_and_handed_back_with_its_claimed_height() {
    let world = world(RequestKind::Movie, &[DUNE_720, DUNE_1080], &[DUNE_1080]).await;
    open_movie_request(&world.host);

    let reply = run(&world, movie_ask()).await;

    let Reply::Imported(imported) = reply else {
        panic!("{reply:?}")
    };
    assert_eq!(imported.request_id, "r1");
    assert_eq!(imported.release.title, DUNE_1080);
    assert_eq!(imported.release.claimed_height, Some(1080));
    assert!(
        imported.primary_path.ends_with(".kremote"),
        "{}",
        imported.primary_path
    );
    assert!(Path::new(&imported.primary_path).starts_with(&world.library));
    assert!(
        world.engine.lock().unwrap().marked.is_empty(),
        "nothing is recorded as imported until the core confirms"
    );
    let conn = world.host.db().get().unwrap();
    assert_eq!(
        db::get_request(&conn, "r1").unwrap().unwrap().status,
        RequestStatus::Approved
    );
    assert!(provisional::get(world.host.store(), &imported.download_id).unwrap().is_some());
}

#[tokio::test]
async fn the_best_scored_release_the_client_holds_wins_over_a_better_one_it_does_not() {
    let world = world(RequestKind::Movie, &[DUNE_1080, DUNE_720], &[DUNE_720]).await;
    open_movie_request(&world.host);

    let reply = run(&world, movie_ask()).await;

    let Reply::Imported(imported) = reply else {
        panic!("{reply:?}")
    };
    assert_eq!(imported.release.title, DUNE_720);
    assert_eq!(offered_titles(&world), [DUNE_1080, DUNE_720]);
}

#[tokio::test]
async fn nothing_held_leaves_the_approved_request_and_says_requested() {
    let world = world(RequestKind::Movie, &[DUNE_1080], &[]).await;
    open_movie_request(&world.host);

    let reply = run(&world, movie_ask()).await;

    assert_eq!(
        reply,
        Reply::Requested {
            request_id: "r1".into()
        }
    );
    let conn = world.host.db().get().unwrap();
    assert_eq!(
        db::get_request(&conn, "r1").unwrap().unwrap().status,
        RequestStatus::Approved
    );
    assert!(files_under(&world.library).is_empty());
}

#[tokio::test]
async fn a_release_the_rules_refuse_is_never_offered_even_when_it_is_the_only_one_held() {
    let world = world(RequestKind::Movie, &[DUNE_CAM, DUNE_1080], &[DUNE_CAM]).await;
    open_movie_request(&world.host);

    let reply = run(&world, movie_ask()).await;

    assert_eq!(
        reply,
        Reply::Requested {
            request_id: "r1".into()
        }
    );
    assert!(
        !offered_titles(&world).iter().any(|t| t == DUNE_CAM),
        "{:?}",
        offered_titles(&world)
    );
    assert!(files_under(&world.library).is_empty());
}

#[tokio::test]
async fn nothing_the_rules_accept_is_unavailable_with_the_request_kept() {
    let world = world(RequestKind::Movie, &[DUNE_CAM], &[DUNE_CAM]).await;
    open_movie_request(&world.host);

    let reply = run(&world, movie_ask()).await;

    assert_eq!(
        reply,
        Reply::Unavailable {
            reason: Why::NoRelease,
            request_id: Some("r1".into())
        }
    );
}

#[tokio::test]
async fn a_client_that_cannot_say_what_it_holds_makes_the_title_unavailable_before_any_search() {
    let world = world(RequestKind::Movie, &[DUNE_1080], &[DUNE_1080]).await;
    open_movie_request(&world.host);
    world.engine.lock().unwrap().capable = false;

    let reply = run(&world, movie_ask()).await;

    assert_eq!(
        reply,
        Reply::Unavailable {
            reason: Why::NoEngine,
            request_id: None
        }
    );
    assert!(offered_titles(&world).is_empty());
}

#[tokio::test]
async fn a_release_the_core_turned_down_is_not_offered_again() {
    let world = world(
        RequestKind::Movie,
        &[DUNE_1080, DUNE_720],
        &[DUNE_1080, DUNE_720],
    )
    .await;
    open_movie_request(&world.host);
    let mut ask = movie_ask();
    ask.exclude = vec![format!("{DUNE_1080}|idx1")];

    let reply = run(&world, ask).await;

    let Reply::Imported(imported) = reply else {
        panic!("{reply:?}")
    };
    assert_eq!(imported.release.title, DUNE_720);
    assert_eq!(offered_titles(&world), [DUNE_720]);
}

#[tokio::test]
async fn an_episode_with_no_instant_release_falls_back_to_the_season_pack_and_returns_that_episode()
{
    let world = world(RequestKind::Show, &[SHOW_E01, SHOW_PACK], &[SHOW_PACK]).await;
    open_episode_request(&world.host);

    let reply = run(&world, episode_ask()).await;

    let Reply::Imported(imported) = reply else {
        panic!("{reply:?}")
    };
    assert_eq!(imported.release.title, SHOW_PACK);
    assert_eq!(
        imported.paths.len(),
        3,
        "every episode of the pack becomes a stub"
    );
    assert!(
        imported.primary_path.contains("S01E01"),
        "{}",
        imported.primary_path
    );
}

#[tokio::test]
async fn a_second_ask_for_an_open_request_makes_no_second_one() {
    let world = world(RequestKind::Movie, &[DUNE_1080], &[]).await;
    open_movie_request(&world.host);

    run(&world, movie_ask()).await;
    run(&world, movie_ask()).await;

    let conn = world.host.db().get().unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM requests", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn a_rejected_release_is_blocklisted_its_files_leave_and_the_download_is_released() {
    let world = world(
        RequestKind::Movie,
        &[DUNE_1080, DUNE_720],
        &[DUNE_1080, DUNE_720],
    )
    .await;
    open_movie_request(&world.host);
    let Reply::Imported(imported) = run(&world, movie_ask()).await else {
        panic!("not imported")
    };
    let host = world.host.clone();
    let rejection = Rejection {
        download_id: imported.download_id.clone(),
        reason: "the file runs 4 minutes".into(),
    };

    assert!(blocking(move || reject(&host, &rejection).unwrap()).await);

    assert!(files_under(&world.library).is_empty());
    let engine = world.engine.lock().unwrap();
    assert!(engine
        .statuses
        .contains(&(imported.download_id.clone(), "removed".into())));
    assert_eq!(engine.discarded, [imported.download_id.clone()]);
    drop(engine);
    let blocked =
        crate::rules::blocklist::for_title(world.host.store(), RequestKind::Movie, 438_631)
            .unwrap();
    assert_eq!(blocked.len(), 1);
    assert_eq!(blocked[0].release_title, DUNE_1080);
    assert!(blocked[0].reason.starts_with("watch now: "));
    let conn = world.host.db().get().unwrap();
    assert_eq!(
        db::get_request(&conn, "r1").unwrap().unwrap().status,
        RequestStatus::Approved
    );
    assert_eq!(
        db::wanted_for_request(&conn, "r1").unwrap()[0].status,
        "wanted"
    );
    assert!(provisional::get(world.host.store(), &imported.download_id).unwrap().is_none());
}

#[tokio::test]
async fn after_a_rejection_the_next_search_refuses_the_blocklisted_release_and_offers_the_next() {
    let world = world(
        RequestKind::Movie,
        &[DUNE_1080, DUNE_720],
        &[DUNE_1080, DUNE_720],
    )
    .await;
    open_movie_request(&world.host);
    let Reply::Imported(first) = run(&world, movie_ask()).await else {
        panic!("not imported")
    };
    let host = world.host.clone();
    let rejection = Rejection {
        download_id: first.download_id,
        reason: "fake".into(),
    };
    blocking(move || reject(&host, &rejection).unwrap()).await;

    let Reply::Imported(second) = run(&world, movie_ask()).await else {
        panic!("not imported")
    };

    assert_eq!(second.release.title, DUNE_720);
}

#[test]
fn a_show_is_asked_for_one_episode_at_a_time() {
    let ask = Ask {
        kind: "show".into(),
        tmdb_id: 42,
        ..Ask::default()
    };

    assert!(asked_unit(RequestKind::Show, &ask).is_err());
    assert_eq!(asked_unit(RequestKind::Movie, &ask).unwrap(), None);
}

#[test]
fn the_reply_crosses_as_a_status_tag_with_snake_case_fields() {
    let imported = serde_json::to_value(Reply::Imported(Imported {
        request_id: "r".into(),
        download_id: "d".into(),
        release: Release {
            key: "k".into(),
            title: "T".into(),
            claimed_height: Some(1080),
        },
        paths: vec!["/a".into()],
        primary_path: "/a".into(),
    }))
    .unwrap();

    assert_eq!(
        imported,
        json!({
            "status": "imported", "request_id": "r", "download_id": "d",
            "release": { "key": "k", "title": "T", "claimed_height": 1080 },
            "paths": ["/a"], "primary_path": "/a"
        })
    );
    assert_eq!(
        serde_json::to_value(Reply::Requested {
            request_id: "r".into()
        })
        .unwrap(),
        json!({ "status": "requested", "request_id": "r" })
    );
    assert_eq!(
        serde_json::to_value(Reply::Present { request_id: None }).unwrap(),
        json!({ "status": "present", "request_id": null })
    );
    assert_eq!(
        serde_json::to_value(Reply::Unavailable {
            reason: Why::NoIndexer,
            request_id: None
        })
        .unwrap(),
        json!({ "status": "unavailable", "reason": "noIndexer", "request_id": null })
    );
}

#[test]
fn an_ask_reads_from_the_core_s_json_and_tolerates_missing_fields() {
    let ask: Ask =
        serde_json::from_value(json!({ "kind": "movie", "tmdb_id": 1, "user_id": "u" })).unwrap();

    assert!(ask.exclude.is_empty());
    assert_eq!(ask.budget_ms, None);
}
